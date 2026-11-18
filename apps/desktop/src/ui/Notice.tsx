import { useState } from "react";
import { api } from "../lib/api";
import { describe, type ErrInfo } from "../lib/errors";
import { useI18n } from "../lib/i18n";
import { useApp } from "./ctx";

/** 错误提示：说清楚出了什么事、怎么修，能修的直接给按钮 */
export function Notice({ err, onClose }: { err: ErrInfo; onClose: () => void }) {
  const { t } = useI18n();
  const app = useApp();
  const [open, setOpen] = useState(false);
  const d = describe(err, t);

  const fix = async () => {
    if (d.action === "connect") {
      onClose();
      await app.connect();
    } else if (d.action === "settings") {
      onClose();
      app.openSettings();
    } else if (d.action) {
      await api.openSystemSettings(d.action);
    }
  };

  return (
    <div className="notice" role="alert">
      <i className="notice-led" aria-hidden />
      <div className="notice-body">
        <strong>{d.title}</strong>
        <p>{d.hint}</p>
        {open && d.detail && <pre className="notice-detail">{d.detail}</pre>}
        <div className="notice-actions">
          {d.action && (
            <button className="btn btn-strong" onClick={fix}>
              {d.action === "connect"
                ? t("common.retry")
                : d.action === "settings"
                  ? t("common.openSettings")
                  : t("common.openSystemSettings")}
            </button>
          )}
          {d.detail && (
            <button className="btn btn-quiet" aria-expanded={open} onClick={() => setOpen((v) => !v)}>
              {t("common.details")}
            </button>
          )}
        </div>
      </div>
      <button className="notice-close" aria-label={t("common.close")} onClick={onClose}>
        <svg viewBox="0 0 12 12" width="12" height="12" aria-hidden>
          <path d="M2 2l8 8M10 2l-8 8" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
        </svg>
      </button>
    </div>
  );
}
