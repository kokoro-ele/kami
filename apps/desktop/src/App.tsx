import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, type Status } from "./lib/api";
import { parseErr, type ErrInfo } from "./lib/errors";
import { dragWindow } from "./lib/window";
import { detectLang, I18nProvider, makeT, type Key } from "./lib/i18n";
import { fontStack, loadSettings, saveSettings, type Settings } from "./lib/settings";
import { Ctx, type AppCtx, type Conn } from "./ui/ctx";
import { Notice } from "./ui/Notice";
import { SettingsSheet } from "./ui/SettingsSheet";
import { CaptionsTab, ImageTab, TextTab, WishTab } from "./ui/tabs";

type Tab = "wish" | "text" | "image" | "captions";
const TABS: Tab[] = ["wish", "text", "image", "captions"];

export default function App() {
  const [settings, setSettingsState] = useState<Settings>(loadSettings);
  const lang = settings.lang === "auto" ? detectLang() : settings.lang;
  const t = useMemo(() => makeT(lang), [lang]);

  const [tab, setTab] = useState<Tab>(() => {
    // 旧版本存的是带引号的 JSON 字符串，读之前先清理并校验
    const saved = (localStorage.getItem("kami.tab") || "").replace(/"/g, "") as Tab;
    return TABS.includes(saved) ? saved : "wish";
  });
  const [status, setStatus] = useState<Status>({ connected: false, name: null });
  const [connecting, setConnecting] = useState(false);
  const [err, setErr] = useState<ErrInfo | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const toastTimer = useRef<number | undefined>(undefined);

  useEffect(() => localStorage.setItem("kami.tab", tab), [tab]);
  useEffect(() => {
    document.documentElement.lang = lang;
  }, [lang]);
  useEffect(() => {
    // 在 Tauri 里运行时窗口是原生磨砂透明的；浏览器预览时用深色底
    const el = document.documentElement;
    if ("__TAURI_INTERNALS__" in window) el.classList.add("native");
    if (/Mac/.test(navigator.platform)) el.classList.add("mac");
  }, []);

  const setSettings = useCallback((s: Settings) => {
    setSettingsState(s);
    saveSettings(s);
  }, []);

  const report = useCallback((e: unknown) => setErr(parseErr(e)), []);
  const showToast = useCallback((msg: string) => {
    setToast(msg);
    window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(null), 2600);
  }, []);

  const connect = useCallback(
    async (quiet = false) => {
      setConnecting(true);
      try {
        setStatus(await api.connect());
        setErr((e) => (e && e.code.startsWith("printer") ? null : e));
      } catch (e) {
        if (!quiet) report(e);
      } finally {
        setConnecting(false);
      }
    },
    [report],
  );

  // 打开就自动连接；之后每 6 秒检查一次，掉线就静默重连
  useEffect(() => {
    connect(true);
    const timer = window.setInterval(async () => {
      try {
        const s = await api.status();
        setStatus(s);
        if (!s.connected) connect(true);
      } catch {
        /* 下次再试 */
      }
    }, 6000);
    return () => window.clearInterval(timer);
  }, [connect]);

  const conn: Conn = connecting ? "connecting" : status.connected ? "on" : "off";

  // 纸张类型（实验）：打开后每次打印开始时告诉打印机用的是连续卷纸
  useEffect(() => {
    api.setPaperMode(settings.continuousPaper ? 2 : null).catch(() => {});
  }, [settings.continuousPaper]);

  // 本机字体：直接读字体文件用 FontFace 加载。
  // WebKit 的 canvas 有时找不到用户自己装的字体，会悄悄换成系统字体（预览有注音、打印没有就是这个原因）
  const [loadedFont, setLoadedFont] = useState<{ family: string; alias: string } | null>(null);
  useEffect(() => {
    const family = settings.font === "local" ? settings.localFont : "";
    if (!family || loadedFont?.family === family) return;
    let alive = true;
    const alias = `Kami Paper ${family}`;
    api
      .fontFile(family)
      .then(async (buf) => {
        const face = new FontFace(alias, buf);
        await face.load();
        document.fonts.add(face);
        if (alive) setLoadedFont({ family, alias });
      })
      .catch(() => {
        /* 字体集合（.ttc）之类的系统字体，canvas 本来就能直接用 */
      });
    return () => {
      alive = false;
    };
  }, [settings.font, settings.localFont]); // eslint-disable-line react-hooks/exhaustive-deps

  const paperFont = useMemo(
    () => ({
      family:
        settings.font === "local" && loadedFont?.family === settings.localFont
          ? `"${loadedFont.alias}", ${fontStack(settings)}`
          : fontStack(settings),
      weight: settings.weight,
      margin: settings.margin,
      lineHeight: settings.lineHeight,
    }),
    [settings.font, settings.localFont, settings.weight, settings.margin, settings.lineHeight, loadedFont], // eslint-disable-line react-hooks/exhaustive-deps
  );

  const ctx: AppCtx = {
    conn,
    printerName: status.name,
    lang,
    settings,
    setSettings,
    paperFont,
    connect: () => connect(false),
    disconnect: async () => {
      await api.disconnect().catch(report);
      setStatus({ connected: false, name: null });
    },
    print: async (png, dither, feed) => {
      try {
        await api.printPng(png, settings.density, dither, feed ?? settings.feedMm * 8);
        setStatus(await api.status());
        return true;
      } catch (e) {
        report(e);
        return false;
      }
    },
    printLine: async (png, dither) => {
      try {
        await api.printLine(png, settings.density, dither);
        return true;
      } catch (e) {
        report(e);
        return false;
      }
    },
    endStream: async (feed) => {
      await api.endStream(settings.density, feed ?? settings.feedMm * 8).catch(report);
    },
    report,
    toast: showToast,
    openSettings: () => setShowSettings(true),
  };

  return (
    <I18nProvider lang={lang}>
      <Ctx.Provider value={ctx}>
        <div className="device">
          <div className="dragbar" onMouseDown={dragWindow} aria-hidden />
          <header className="top" onMouseDown={dragWindow}>
            <h1 className="mark">
              紙<span>kami</span>
            </h1>
            <div className={`conn conn-${conn}`} aria-live="polite">
              <i />
              {conn === "on" ? status.name : conn === "connecting" ? t("conn.connecting") : t("conn.off")}
            </div>
            <button className="gear" aria-label={t("settings.title")} onClick={() => setShowSettings(true)}>
              <svg viewBox="0 0 20 20" width="18" height="18" aria-hidden>
                <path d="M3 5.5h14M3 10h14M3 14.5h14" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
                <circle cx="7" cy="5.5" r="1.9" fill="#16181b" stroke="currentColor" strokeWidth="1.6" />
                <circle cx="13" cy="10" r="1.9" fill="#16181b" stroke="currentColor" strokeWidth="1.6" />
                <circle cx="9" cy="14.5" r="1.9" fill="#16181b" stroke="currentColor" strokeWidth="1.6" />
              </svg>
            </button>
          </header>

          <nav className="tabs" role="tablist">
            {TABS.map((k) => (
              <button key={k} role="tab" aria-selected={tab === k} onClick={() => setTab(k)}>
                {t(`tab.${k}` as Key)}
              </button>
            ))}
          </nav>

          {err && <Notice err={err} onClose={() => setErr(null)} />}

          {/* 所有页面都保持挂载：切到别的页面时字幕继续收听，输入也不会丢 */}
          <div className="tabpane" hidden={tab !== "wish"}>
            <WishTab />
          </div>
          <div className="tabpane" hidden={tab !== "text"}>
            <TextTab />
          </div>
          <div className="tabpane" hidden={tab !== "image"}>
            <ImageTab />
          </div>
          <div className="tabpane" hidden={tab !== "captions"}>
            <CaptionsTab />
          </div>

          {showSettings && <SettingsSheet onClose={() => setShowSettings(false)} />}

          {toast && (
            <div className="toast" role="status">
              {toast}
            </div>
          )}
        </div>
      </Ctx.Provider>
    </I18nProvider>
  );
}
