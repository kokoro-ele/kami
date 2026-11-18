import { useEffect, useMemo, useState } from "react";
import { api, type FontInfo } from "../lib/api";
import { describe, parseErr } from "../lib/errors";
import { lint } from "../lib/format";
import { dragWindow } from "../lib/window";
import { useI18n, type Key } from "../lib/i18n";
import { AI_DEFAULTS, applyTemplate, fontStack, formatDate, formatTime, type FontId, type Settings } from "../lib/settings";
import { Row, Seg, Switch } from "./controls";
import { useApp } from "./ctx";

const FONTS: FontId[] = ["sans", "serif", "kai", "round", "local"];

export function SettingsSheet({ onClose }: { onClose: () => void }) {
  const { t } = useI18n();
  const app = useApp();
  const s = app.settings;
  const set = (patch: Partial<Settings>) => app.setSettings({ ...s, ...patch });
  const setRules = (patch: Partial<Settings["rules"]>) => set({ rules: { ...s.rules, ...patch } });
  const setAi = (patch: Partial<Settings["ai"]>) => set({ ai: { ...s.ai, ...patch } });

  return (
    <div className="sheet" role="dialog" onMouseDown={(e) => e.target === e.currentTarget && dragWindow(e)} aria-modal="true" aria-label={t("settings.title")}>
      <header className="sheet-head" onMouseDown={dragWindow}>
        <h2>{t("settings.title")}</h2>
        <button className="btn btn-strong" onClick={onClose} autoFocus>
          {t("common.done")}
        </button>
      </header>

      <div className="sheet-body">
        {/* ------------------------------------------------ 打印机 */}
        <section className="group">
          <h3>{t("settings.printer")}</h3>
          <div className="rows">
          <Row label={app.conn === "on" ? t("settings.connectedTo") : t("settings.notConnected")}>
            {app.conn === "on" ? (
              <span className="inline">
                <b>{app.printerName}</b>
                <button className="btn btn-quiet" onClick={app.disconnect}>
                  {t("settings.disconnect")}
                </button>
              </span>
            ) : (
              <button className="btn btn-strong" onClick={app.connect}>
                {t("press.connect")}
              </button>
            )}
          </Row>
          <Row label={t("settings.density")}>
            <Seg
              label={t("settings.density")}
              value={s.density}
              onChange={(density) => set({ density })}
              options={([0, 1, 2] as const).map((v) => [v, t(`settings.density.${v}` as Key)])}
            />
          </Row>
          <Row label={t("settings.feed")} help={t("settings.feed.help")}>
            <Seg
              label={t("settings.feed")}
              value={s.feedMm}
              onChange={(feedMm) => set({ feedMm })}
              options={([0, 3, 5, 10, 15] as const).map((v) => [v, String(v)])}
              small
            />
          </Row>
          <Row
            label={
              <>
                {t("settings.continuous")}
                <span className="badge">{t("common.experimental")}</span>
              </>
            }
            help={t("settings.continuous.help")}
          >
            <Switch label={t("settings.continuous")} on={s.continuousPaper} onChange={(continuousPaper) => set({ continuousPaper })} />
          </Row>
          <Row label={t("settings.dither")}>
            <Seg
              label={t("settings.dither")}
              value={s.dither}
              onChange={(dither) => set({ dither })}
              options={(["fs", "atkinson", "threshold"] as const).map((v) => [v, t(`settings.dither.${v}` as Key)])}
            />
          </Row>
          </div>
        </section>

        {/* ------------------------------------------------ 纸面 */}
        <section className="group">
          <h3>{t("settings.paper")}</h3>
          <div className="rows">
          <Row label={t("settings.font")} stack>
            <div className="fonts" role="radiogroup" aria-label={t("settings.font")}>
              {FONTS.map((f) => (
                <button
                  key={f}
                  role="radio"
                  aria-checked={s.font === f}
                  onClick={() => set({ font: f })}
                  style={{ fontFamily: fontStack({ font: f, localFont: s.localFont }) }}
                >
                  <span className="font-sample">紙 Aa</span>
                  <small>{f === "local" && s.localFont ? s.localFont : t(`settings.font.${f}` as Key)}</small>
                </button>
              ))}
            </div>
            {s.font === "local" && <FontPicker value={s.localFont} onPick={(localFont) => set({ localFont })} />}
          </Row>
          <Row label={t("settings.weight")}>
            <Seg
              label={t("settings.weight")}
              value={s.weight}
              onChange={(weight) => set({ weight })}
              options={[
                [400, t("settings.weight.400")],
                [700, t("settings.weight.700")],
              ]}
            />
          </Row>
          <Row label={t("settings.textDither")} help={t("settings.textDither.help")}>
            <Seg
              label={t("settings.textDither")}
              value={s.textDither}
              onChange={(textDither) => set({ textDither })}
              options={[
                ["sharp", t("settings.textDither.sharp")],
                ["detail", t("settings.textDither.detail")],
                ["bold", t("settings.textDither.bold")],
              ]}
            />
          </Row>
          <Row label={t("settings.margin")} help={t("settings.margin.help")}>
            <Seg
              label={t("settings.margin")}
              value={s.margin}
              onChange={(margin) => set({ margin })}
              options={([0, 8, 16, 24] as const).map((v) => [v, t(`settings.margin.${v}` as Key)])}
            />
          </Row>
          <Row label={t("settings.timestamp")} help={t("settings.timestamp.help")}>
            <Switch label={t("settings.timestamp")} on={s.timestamp} onChange={(timestamp) => set({ timestamp })} />
          </Row>
          {s.timestamp && (
            <Row label={t("settings.timeFormat")}>
              <Seg
                label={t("settings.timeFormat")}
                value={s.timeFormat}
                onChange={(timeFormat) => set({ timeFormat })}
                options={[
                  ["HH:mm:ss", "12:34:56"],
                  ["HH:mm", "12:34"],
                ]}
              />
            </Row>
          )}
          <Row label={t("settings.template")} help={t("settings.template.help")} stack>
            <input className="field" value={s.template} onChange={(e) => set({ template: e.target.value })} spellCheck={false} />
            <p className="sample">
              {applyTemplate(s.template, {
                text: t("text.placeholder"),
                time: formatTime(new Date(), s.timeFormat),
                date: formatDate(new Date()),
                n: 1,
              })}
            </p>
          </Row>
          </div>
        </section>

        {/* ------------------------------------------------ 文本整理 */}
        <section className="group">
          <h3>{t("settings.format")}</h3>
          <p className="group-help">{t("settings.format.help")}</p>
          <div className="rows">
          {(["whitespace", "fillers", "pangu", "punctuation"] as const).map((k) => (
            <Row key={k} label={t(`settings.rule.${k}` as Key)}>
              <Switch
                label={t(`settings.rule.${k}` as Key)}
                on={s.rules[k]}
                onChange={(v) => setRules({ [k]: v } as Partial<Settings["rules"]>)}
              />
            </Row>
          ))}
          <Row label={t("settings.rule.custom")} help={t("settings.rule.custom.help")} stack>
            <textarea
              className="field mono"
              rows={3}
              value={s.rules.custom}
              placeholder={"Kami => 紙\n/(\\d+)点/ => $1 時"}
              spellCheck={false}
              onChange={(e) => setRules({ custom: e.target.value })}
            />
          </Row>
          <LintPlayground rules={s.rules} />
          </div>
        </section>

        {/* ------------------------------------------------ AI */}
        <section className="group">
          <h3>{t("settings.ai")}</h3>
          <p className="group-help">{t("settings.ai.help")}</p>
          <div className="rows">
          <Row label={t("settings.ai.provider")} help={s.ai.provider === "openai" ? t("settings.ai.openai.help") : undefined}>
            <Seg
              label={t("settings.ai.provider")}
              value={s.ai.provider}
              onChange={(provider) => setAi({ provider, ...AI_DEFAULTS[provider] })}
              options={[
                ["anthropic", t("settings.ai.anthropic")],
                ["openai", t("settings.ai.openai")],
              ]}
            />
          </Row>
          <Row label={t("settings.ai.baseUrl")} stack>
            <input className="field" value={s.ai.baseUrl} spellCheck={false} onChange={(e) => setAi({ baseUrl: e.target.value })} />
          </Row>
          <Row label={t("settings.ai.model")} stack>
            <input className="field" value={s.ai.model} spellCheck={false} onChange={(e) => setAi({ model: e.target.value })} />
          </Row>
          <Row label={t("settings.ai.key")} stack>
            <input
              className="field"
              type="password"
              value={s.ai.apiKey}
              placeholder={s.ai.provider === "anthropic" ? "sk-ant-…" : "sk-…"}
              onChange={(e) => setAi({ apiKey: e.target.value })}
            />
          </Row>
          <AiTest />
          </div>
        </section>

        {/* ------------------------------------------------ 语言 */}
        <section className="group">
          <h3>{t("settings.general")}</h3>
          <div className="rows">
          <Row label={t("settings.language")}>
            <Seg
              label={t("settings.language")}
              value={s.lang}
              onChange={(lang) => set({ lang })}
              options={[
                ["auto", t("settings.language.auto")],
                ["zh", "中文"],
                ["ja", "日本語"],
                ["en", "English"],
              ]}
              small
            />
          </Row>
          </div>
        </section>
      </div>
    </div>
  );
}

function LintPlayground({ rules }: { rules: Settings["rules"] }) {
  const { t, lang } = useI18n();
  const sample = {
    zh: "嗯，今天我们 聊一聊 iPhone17的新功能,好吗?",
    ja: "えーと、明日は10時に渋谷で会いましょう",
    en: "um, so we shipped kami today ,  finally",
  }[lang];
  const [text, setText] = useState(sample);
  const r = useMemo(() => lint(text, rules), [text, rules]);
  return (
    <div className="playground">
      <label className="setrow-label">
        <span>{t("settings.format.try")}</span>
      </label>
      <textarea className="field" rows={2} value={text} onChange={(e) => setText(e.target.value)} />
      <div className="lint-out">
        <span className={`lint-count ${r.fixes ? "has" : ""}`}>
          {r.fixes ? t("settings.format.result", { n: r.fixes }) : t("settings.format.clean")}
        </span>
        <p>{r.text}</p>
        {r.badRules.length > 0 && <p className="lint-bad">✕ {r.badRules.join("　")}</p>}
      </div>
    </div>
  );
}

function AiTest() {
  const { t } = useI18n();
  const { settings } = useApp();
  const [state, setState] = useState<"idle" | "testing" | "ok" | { title: string; detail: string }>("idle");
  const run = async () => {
    setState("testing");
    try {
      await api.aiComplete(settings.ai, "Reply with the single word: OK");
      setState("ok");
    } catch (e) {
      const d = describe(parseErr(e), t);
      setState({ title: d.title, detail: d.detail });
    }
  };
  return (
    <div className="ai-test">
      <button className="btn btn-quiet" onClick={run} disabled={state === "testing"}>
        {state === "testing" ? t("settings.ai.testing") : t("settings.ai.test")}
      </button>
      {state === "ok" && <span className="ok">● {t("settings.ai.ok")}</span>}
      {typeof state === "object" && (
        <span className="bad" title={state.detail}>
          ● {state.title}
        </span>
      )}
    </div>
  );
}

/** 本机字体列表：可搜索，可只看中日文字体，每一项用它自己的字体显示 */
function FontPicker({ value, onPick }: { value: string; onPick: (family: string) => void }) {
  const { t } = useI18n();
  const [fonts, setFonts] = useState<FontInfo[] | null>(null);
  const [q, setQ] = useState("");
  const [cjkOnly, setCjkOnly] = useState(true);

  useEffect(() => {
    api.listFonts().then(setFonts).catch(() => setFonts([]));
  }, []);

  const list = useMemo(() => {
    const needle = q.trim().toLowerCase();
    return (fonts ?? []).filter(
      (f) =>
        (!cjkOnly || f.cjk) &&
        (!needle || f.family.toLowerCase().includes(needle) || (f.alias ?? "").toLowerCase().includes(needle)),
    );
  }, [fonts, q, cjkOnly]);

  return (
    <div className="fontpicker">
      <div className="fontpicker-bar">
        <input className="field" value={q} placeholder={t("settings.font.search")} onChange={(e) => setQ(e.target.value)} />
        <label className="check">
          <Switch label={t("settings.font.cjkOnly")} on={cjkOnly} onChange={setCjkOnly} />
          <span>{t("settings.font.cjkOnly")}</span>
        </label>
      </div>
      <div className="fontlist" role="listbox" aria-label={t("settings.font.pick")}>
        {fonts === null && <p className="fontlist-empty">{t("settings.font.loading")}</p>}
        {fonts !== null && list.length === 0 && <p className="fontlist-empty">{t("settings.font.none")}</p>}
        {list.map((f) => (
          <button
            key={f.family}
            role="option"
            aria-selected={value === f.family}
            onClick={() => onPick(f.family)}
            style={{ fontFamily: `"${f.family}"` }}
          >
            <span className="fontlist-sample">紙 Aa 永</span>
            <span className="fontlist-name">
              {f.family}
              {f.alias && <small>{f.alias}</small>}
            </span>
          </button>
        ))}
      </div>
      {fonts && <p className="fontlist-count">{t("settings.font.count", { n: list.length })}</p>}
    </div>
  );
}
