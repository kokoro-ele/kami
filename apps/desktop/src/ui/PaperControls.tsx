import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { api, type FontInfo } from "../lib/api";
import { useI18n, type Key } from "../lib/i18n";
import type { FontId, Settings } from "../lib/settings";
import { WIDTH } from "../lib/sheet";
import { useApp } from "./ctx";

const PRESETS: Exclude<FontId, "local">[] = ["sans", "serif", "kai", "round"];

let fontCache: FontInfo[] | null = null;

/** 文字页、字幕页上方的纸面工具条：字体 / 字号 / 粗体 / 时间戳，和设置页是同一份设置 */
export function PaperControls({
  sizeKey,
  showTime = true,
  extra,
}: {
  sizeKey: "textSize" | "capSize";
  showTime?: boolean;
  extra?: ReactNode;
}) {
  const { t } = useI18n();
  const app = useApp();
  const s = app.settings;
  const set = (patch: Partial<Settings>) => app.setSettings({ ...s, ...patch });
  const [fonts, setFonts] = useState<FontInfo[]>(fontCache ?? []);

  useEffect(() => {
    if (fontCache) return;
    api
      .listFonts()
      .then((list) => {
        fontCache = list.filter((f) => f.cjk);
        setFonts(fontCache);
      })
      .catch(() => {});
  }, []);

  const value = s.font === "local" ? `local:${s.localFont}` : `preset:${s.font}`;
  const size = s[sizeKey];

  return (
    <div className="papertools" role="group" aria-label={t("settings.paper")}>
      <select
        className="field select papertools-font"
        aria-label={t("paper.font")}
        value={value}
        style={{ fontFamily: app.paperFont.family }}
        onChange={(e) => {
          const [kind, name] = e.target.value.split(/:(.*)/s);
          if (kind === "preset") set({ font: name as FontId });
          else set({ font: "local", localFont: name });
        }}
      >
        <optgroup label={t("paper.presets")}>
          {PRESETS.map((f) => (
            <option key={f} value={`preset:${f}`}>
              {t(`settings.font.${f}` as Key)}
            </option>
          ))}
        </optgroup>
        {(fonts.length > 0 || s.font === "local") && (
          <optgroup label={t("paper.installed")}>
            {s.font === "local" && !fonts.some((f) => f.family === s.localFont) && (
              <option value={`local:${s.localFont}`}>{s.localFont}</option>
            )}
            {fonts.map((f) => (
              <option key={f.family} value={`local:${f.family}`}>
                {f.alias ? `${f.family}（${f.alias}）` : f.family}
              </option>
            ))}
          </optgroup>
        )}
      </select>

      <div className="papertools-row">
      <div className="stepper" aria-label={t("paper.size")}>
        <button onClick={() => set({ [sizeKey]: Math.max(14, size - 2) } as Partial<Settings>)} aria-label="−">
          −
        </button>
        <output title={t("paper.size")}>{size}</output>
        <button onClick={() => set({ [sizeKey]: Math.min(80, size + 2) } as Partial<Settings>)} aria-label="+">
          +
        </button>
      </div>

      <div className="stepper" aria-label={t("paper.lineHeight")} title={t("paper.lineHeight")}>
        <button
          onClick={() => set({ lineHeight: Math.max(1, Math.round((s.lineHeight - 0.1) * 10) / 10) })}
          aria-label="−"
        >
          −
        </button>
        <output className="lh">
          <LineHeightIcon />
          {s.lineHeight.toFixed(1)}
        </output>
        <button
          onClick={() => set({ lineHeight: Math.min(2.5, Math.round((s.lineHeight + 0.1) * 10) / 10) })}
          aria-label="+"
        >
          +
        </button>
      </div>

      <button
        className="chip"
        aria-pressed={s.weight === 700}
        onClick={() => set({ weight: s.weight === 700 ? 400 : 700 })}
        title={t("paper.bold")}
      >
        <b>B</b>
      </button>
      {showTime && (
        <button className="chip" aria-pressed={s.timestamp} onClick={() => set({ timestamp: !s.timestamp })}>
          {t("paper.time")}
        </button>
      )}
      {extra && <div className="papertools-extra">{extra}</div>}
      </div>
    </div>
  );
}

/** 按纸宽等比缩放：里面按 384 点排版，外面缩放到实际纸条宽度，所见即所得 */
export function PaperScale({ children }: { children: ReactNode }) {
  const outer = useRef<HTMLDivElement>(null);
  const [zoom, setZoom] = useState(1);
  useLayoutEffect(() => {
    const el = outer.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setZoom(el.clientWidth / WIDTH));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  return (
    <div ref={outer} className="paperscale">
      <div style={{ width: WIDTH, zoom }}>{children}</div>
    </div>
  );
}

function LineHeightIcon() {
  return (
    <svg viewBox="0 0 14 14" width="12" height="12" aria-hidden>
      <path d="M5 3h8M5 7h8M5 11h8M2 2v10M1 3l1-1.5L3 3M1 11l1 1.5L3 11" stroke="currentColor" strokeWidth="1.3" fill="none" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}
