import { useEffect, useState, type ReactNode } from "react";
import { api } from "../lib/api";
import { useI18n } from "../lib/i18n";
import { useApp } from "./ctx";

export type Action = {
  label: string;
  onPress: () => Promise<unknown> | void;
  disabled?: boolean;
  /** 停止类操作：红色灯环，不需要先连接打印机 */
  stop?: boolean;
  /** 不需要打印机也能按（比如抽签、开始收听但不自动打印） */
  offline?: boolean;
};

/** 出纸口 + 纸条 + 输入区 + 大按键 */
export function Machine({
  paper,
  composer,
  action,
  feedKey,
  meter,
}: {
  paper: ReactNode;
  composer: ReactNode;
  action: Action;
  feedKey?: number;
  meter?: number | null;
}) {
  const { t } = useI18n();
  const app = useApp();
  const [busy, setBusy] = useState(false);
  const free = action.stop || action.offline;
  const needsConnect = app.conn !== "on" && !free;
  const ring = action.stop ? "stop" : busy || (app.conn === "connecting" && !free) ? "busy" : app.conn === "on" || free ? "on" : "off";
  const label = busy
    ? t("press.wait")
    : needsConnect
      ? app.conn === "connecting"
        ? t("press.connecting")
        : t("press.connect")
      : action.label;

  const press = async () => {
    setBusy(true);
    try {
      if (needsConnect) await app.connect();
      else await action.onPress();
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <section className="slot-wrap">
        <div className="slot" aria-hidden>
          {meter != null && <span className="meter" style={{ transform: `scaleX(${Math.min(1, meter * 14)})` }} />}
        </div>
        <div className="feed">
          <div className="paper" key={feedKey}>
            {paper}
          </div>
        </div>
      </section>
      {composer && <section className="composer">{composer}</section>}
      <footer className="base">
        <button
          className={`press ring-${ring}`}
          onClick={press}
          disabled={busy || (needsConnect && app.conn === "connecting") || (!needsConnect && action.disabled)}
        >
          <span className="cap" />
        </button>
        <span className="press-label">{label}</span>
      </footer>
    </>
  );
}

/** 灰度图 → 后端抖动后的真实打印效果 */
export function Preview({ png, dither, empty }: { png: string | null; dither: string; empty: ReactNode }) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    let alive = true;
    if (!png) {
      setSrc(null);
      return;
    }
    api
      .previewPng(png, dither)
      .then((s) => alive && setSrc(s))
      .catch(() => alive && setSrc(png));
    return () => {
      alive = false;
    };
  }, [png, dither]);
  return src ? <img className="print" src={src} alt="" /> : <p className="blank">{empty}</p>;
}
