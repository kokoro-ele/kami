//! 蓝牙传输：扫描、连接、发送打印任务（btleplug，跨 macOS / Windows / Linux）。

use std::time::Duration;

use btleplug::api::{
    Central, Characteristic, Manager as _, Peripheral as _, ScanFilter, WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral};
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout_at, Instant};
use uuid::Uuid;

use crate::protocol::{driver_for, Driver, JobOptions};
use crate::raster::Bitmap;
use crate::{err, Error, Result};

#[derive(Clone, Debug)]
pub struct DeviceInfo {
    pub name: String,
    pub id: String,
    pub rssi: Option<i16>,
    /// 是否有内置驱动支持
    pub supported: bool,
}

#[derive(Clone, Debug)]
pub struct ConnectOptions {
    /// 只连名字以此开头的设备；未知名字也会按 LuckJingle 协议尝试
    pub name_prefix: Option<String>,
    pub timeout: Duration,
}

impl Default for ConnectOptions {
    fn default() -> Self {
        Self {
            name_prefix: None,
            timeout: Duration::from_secs(12),
        }
    }
}

async fn adapter() -> Result<Adapter> {
    let manager = Manager::new().await?;
    manager
        .adapters()
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| Error::from("找不到蓝牙适配器（蓝牙打开了吗？）"))
}

fn short_id(p: &Peripheral) -> String {
    let s = format!("{:?}", p.id());
    s.trim_start_matches("PeripheralId(")
        .trim_end_matches(')')
        .to_string()
}

/// 扫描附近所有带名字的 BLE 设备，支持的打印机排在前面
pub async fn scan(duration: Duration, extra_prefixes: &[String]) -> Result<Vec<DeviceInfo>> {
    let central = adapter().await?;
    central.start_scan(ScanFilter::default()).await?;
    sleep(duration).await;
    let mut out = Vec::new();
    for p in central.peripherals().await? {
        let Some(props) = p.properties().await? else {
            continue;
        };
        let Some(name) = props.local_name else {
            continue;
        };
        out.push(DeviceInfo {
            supported: driver_for(&name, extra_prefixes).is_some(),
            id: short_id(&p),
            rssi: props.rssi,
            name,
        });
    }
    central.stop_scan().await.ok();
    out.sort_by_key(|d| (!d.supported, -(d.rssi.unwrap_or(-127) as i32)));
    Ok(out)
}

pub struct Printer {
    peripheral: Peripheral,
    write_char: Characteristic,
    driver: Box<dyn Driver>,
    name: String,
    notes: mpsc::UnboundedReceiver<Vec<u8>>,
    listener: JoinHandle<()>,
}

impl Printer {
    /// 扫描并连接第一台受支持的打印机
    pub async fn connect(opts: &ConnectOptions) -> Result<Self> {
        let extra: Vec<String> = opts.name_prefix.iter().cloned().collect();
        let central = adapter().await?;
        central.start_scan(ScanFilter::default()).await?;
        let deadline = Instant::now() + opts.timeout;

        let found = 'search: loop {
            for p in central.peripherals().await? {
                let Some(props) = p.properties().await? else {
                    continue;
                };
                let Some(name) = props.local_name else {
                    continue;
                };
                if let Some(pre) = &opts.name_prefix {
                    if !name.starts_with(pre.as_str()) {
                        continue;
                    }
                }
                if let Some(d) = driver_for(&name, &extra) {
                    break 'search Some((p, name, d));
                }
            }
            if Instant::now() >= deadline {
                break None;
            }
            sleep(Duration::from_millis(300)).await;
        };
        central.stop_scan().await.ok();

        let Some((peripheral, name, driver)) = found else {
            return err("没找到打印机：确认已开机，且手机 App 已断开连接");
        };

        peripheral.connect().await?;
        peripheral.discover_services().await?;
        let chars = peripheral.characteristics();
        let find = |u: u128| chars.iter().find(|c| c.uuid == Uuid::from_u128(u)).cloned();

        let write_char = find(driver.write_uuid())
            .ok_or_else(|| Error::from(format!("{name} 上没有找到写入特征值")))?;
        for u in driver.notify_uuids() {
            if let Some(c) = find(u) {
                peripheral.subscribe(&c).await?;
            }
        }

        let mut stream = peripheral.notifications().await?;
        let (tx, notes) = mpsc::unbounded_channel();
        let listener = tokio::spawn(async move {
            while let Some(n) = stream.next().await {
                if tx.send(n.value).is_err() {
                    break;
                }
            }
        });

        Ok(Self {
            peripheral,
            write_char,
            driver,
            name,
            notes,
            listener,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn driver_name(&self) -> &'static str {
        self.driver.name()
    }

    /// 打印头宽度（点）
    pub fn width(&self) -> usize {
        self.driver.width()
    }

    pub async fn is_connected(&self) -> bool {
        self.peripheral.is_connected().await.unwrap_or(false)
    }

    async fn write_cmds(&self, cmds: Vec<Vec<u8>>) -> Result<()> {
        let chunk = self.driver.chunk_size();
        let pace = Duration::from_millis(self.driver.pace_ms());
        for cmd in cmds {
            for part in cmd.chunks(chunk) {
                self.peripheral
                    .write(&self.write_char, part, WriteType::WithResponse)
                    .await?;
                sleep(pace).await;
            }
        }
        Ok(())
    }

    fn check_width(&self, bmp: &Bitmap) -> Result<()> {
        if bmp.width != self.width() {
            return err(format!(
                "位图宽度 {} ≠ 打印头宽度 {}",
                bmp.width,
                self.width()
            ));
        }
        Ok(())
    }

    /// 发送一张已经抖动好的位图（宽度必须等于 `width()`），一次完整的打印任务
    pub async fn print(&mut self, bmp: &Bitmap, opts: &JobOptions) -> Result<()> {
        self.check_width(bmp)?;
        while self.notes.try_recv().is_ok() {}
        self.write_cmds(self.driver.encode(bmp, opts)).await?;

        // 等待回执；没等到也不算失败（部分固件不回）
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            match timeout_at(deadline, self.notes.recv()).await {
                Ok(Some(d)) if self.driver.is_done_ack(&d) => return Ok(()),
                Ok(Some(_)) => continue,
                Ok(None) => return err("打印过程中连接断开"),
                Err(_) => return Ok(()),
            }
        }
    }

    /// 连续打印：开始一次任务。之后用 `stream_raster` 一段段发，最后 `stream_end`。
    /// 每段之间不会走纸，字幕逐行打印时行与行之间不会出现空白。
    pub async fn stream_begin(&mut self, opts: &JobOptions) -> Result<()> {
        while self.notes.try_recv().is_ok() {}
        self.write_cmds(self.driver.encode_begin(opts)).await
    }

    pub async fn stream_raster(&mut self, bmp: &Bitmap) -> Result<()> {
        self.check_width(bmp)?;
        self.write_cmds(self.driver.encode_raster(bmp)).await
    }

    pub async fn stream_end(&mut self, opts: &JobOptions) -> Result<()> {
        self.write_cmds(self.driver.encode_end(opts)).await
    }

    pub async fn disconnect(self) -> Result<()> {
        self.listener.abort();
        self.peripheral.disconnect().await?;
        Ok(())
    }
}

impl Drop for Printer {
    fn drop(&mut self) {
        self.listener.abort();
    }
}
