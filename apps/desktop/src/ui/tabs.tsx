import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, events } from "../lib/api";
import { lint } from "../lib/format";
import { fortuneCard, makeFortune } from "../lib/fortune";
import { useI18n, type Key } from "../lib/i18n";
import { applyTemplate, formatDate, formatTime, textDitherOf, type CaptionMode } from "../lib/settings";
import { fileToDataUrl, lineBox, paperBreaks, tapeCard, textCard } from "../lib/sheet";
import { Seg } from "./controls";
import { useApp } from "./ctx";
import { Machine, Preview } from "./Machine";
import { PaperControls, PaperScale } from "./PaperControls";

const loadNum = (k: string) => Number(localStorage.getItem(k) || 0);

// ------------------------------------------------------------------ 许愿
export function WishTab() {
  const { t, lang } = useI18n();
  const app = useApp();
  const [wish, setWish] = useState("");
  const [png, setPng] = useState<string | null>(null);
  const [drawnFor, setDrawnFor] = useState("");
  const [feed, setFeed] = useState(0);
  const [drawing, setDrawing] = useState(false);

  const draw = async () => {
    const w = wish.trim();
    if (!w || drawing) return;
    setDrawing(true);
    try {
      const { fortune, error } = await makeFortune(w, lang, app.settings.ai);
      if (error) app.report(error);
      const serial = loadNum("kami.serial") + 1;
      localStorage.setItem("kami.serial", String(serial));
      setPng(fortuneCard(w, fortune, serial, lang, app.paperFont));
      setDrawnFor(w);
    } finally {
      setDrawing(false);
    }
  };

  const ready = Boolean(png && drawnFor === wish.trim());
  return (
    <Machine
      feedKey={feed}
      paper={<Preview png={png} dither={textDitherOf(app.settings)} empty={drawing ? t("wish.drawing") : t("wish.empty")} />}
      composer={
        <input
          className="field"
          value={wish}
          placeholder={t("wish.placeholder")}
          onChange={(e) => setWish(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && !e.nativeEvent.isComposing && draw()}
        />
      }
      action={
        ready
          ? {
              label: t("wish.print"),
              onPress: async () => {
                if (await app.print(png!, textDitherOf(app.settings))) {
                  setFeed((n) => n + 1);
                  setPng(null);
                  setWish("");
                  app.toast(t("wish.printed"));
                }
              },
            }
          : { label: drawing ? t("wish.drawing") : t("wish.draw"), onPress: draw, disabled: !wish.trim() || drawing, offline: true }
      }
    />
  );
}

// ------------------------------------------------------------------ 文字
export function TextTab() {
  const { t } = useI18n();
  const app = useApp();
  const s = app.settings;
  const [text, setText] = useState("");
  const size = s.textSize;
  const [center, setCenter] = useState(false);
  const [feed, setFeed] = useState(0);

  const { png, fixes } = useMemo(() => {
    if (!text.trim()) return { png: null, fixes: 0 };
    const r = lint(text, s.rules);
    const now = new Date();
    const body = r.text
      .split("\n")
      .map((line, i) =>
        line.trim()
          ? applyTemplate(s.template, { text: line, time: formatTime(now, s.timeFormat), date: formatDate(now), n: i + 1 })
          : line,
      )
      .join("\n");
    const stamp = s.timestamp ? `${formatDate(now)} ${formatTime(now, s.timeFormat)}` : undefined;
    return { png: textCard(body, size, center, app.paperFont, stamp), fixes: r.fixes };
    // 时间戳精确到分钟就够了，不需要每秒重新渲染
  }, [text, size, center, s.rules, s.template, s.timestamp, s.timeFormat, app.paperFont]);

  return (
    <Machine
      feedKey={feed}
      paper={<Preview png={png} dither={textDitherOf(s)} empty={t("text.empty")} />}
      composer={
        <>
          <PaperControls
            sizeKey="textSize"
            extra={
              <Seg
                label={t("text.center")}
                value={center ? "c" : "l"}
                onChange={(v) => setCenter(v === "c")}
                options={[
                  ["l", <AlignIcon key="l" center={false} />],
                  ["c", <AlignIcon key="c" center />],
                ]}
                small
              />
            }
          />
          <textarea className="field" value={text} rows={3} placeholder={t("text.placeholder")} onChange={(e) => setText(e.target.value)} />
          {fixes > 0 && <p className="meta">{t("text.fixes", { n: fixes })}</p>}
        </>
      }
      action={{
        label: t("text.print"),
        disabled: !png,
        onPress: async () => {
          if (png && (await app.print(png, textDitherOf(s)))) {
            setFeed((n) => n + 1);
            app.toast(t("common.printed"));
          }
        },
      }}
    />
  );
}

function AlignIcon({ center }: { center: boolean }) {
  return (
    <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden>
      {(center ? [[3, 13], [5, 11], [2, 14]] : [[2, 14], [2, 10], [2, 12]]).map(([a, b], i) => (
        <path key={i} d={`M${a} ${4 + i * 4}H${b}`} stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
      ))}
    </svg>
  );
}

// ------------------------------------------------------------------ 图片
export function ImageTab() {
  const { t } = useI18n();
  const app = useApp();
  const [png, setPng] = useState<string | null>(null);
  const [over, setOver] = useState(false);
  const [feed, setFeed] = useState(0);
  const input = useRef<HTMLInputElement>(null);

  const take = async (f?: File | null) => {
    if (!f) return;
    if (!f.type.startsWith("image/")) return app.report({ code: "bad_image", detail: f.name });
    setPng(await fileToDataUrl(f));
  };

  return (
    <div
      className={`dropzone ${over ? "over" : ""}`}
      onDragOver={(e) => {
        e.preventDefault();
        setOver(true);
      }}
      onDragLeave={(e) => !e.currentTarget.contains(e.relatedTarget as Node | null) && setOver(false)}
      onDrop={(e) => {
        e.preventDefault();
        setOver(false);
        take(e.dataTransfer.files[0]);
      }}
    >
      <Machine
        feedKey={feed}
        paper={<Preview png={png} dither={app.settings.dither} empty={over ? t("image.drop") : t("image.empty")} />}
        composer={
          <div className="row">
            <button className="btn btn-quiet grow" onClick={() => input.current?.click()}>
              {png ? t("image.change") : t("image.choose")}
            </button>
            <input ref={input} type="file" accept="image/*" hidden onChange={(e) => take(e.target.files?.[0])} />
          </div>
        }
        action={{
          label: t("image.print"),
          disabled: !png,
          onPress: async () => {
            if (png && (await app.print(png, app.settings.dither))) {
              setFeed((n) => n + 1);
              app.toast(t("common.printed"));
            }
          },
        }}
      />
    </div>
  );
}

// ------------------------------------------------------------------ 实时字幕
//
// 一句话（utterance）从识别器的临时结果开始，到最终结果结束。
// 「逐行」：正在说的这句话，排满的行只要在 STABLE_MS 内没被识别器改过，就立刻打印这一行；
// 句子结束时把剩下的半行也打掉。每句话记录「已经打到第几个字」（printed）。

const STABLE_MS = 900;

type Snap = { t: number; text: string };
type Utt = {
  id: number;
  time: string;
  /** 整理 + 套模板后的文字（就是会打在纸上的文字） */
  text: string;
  final: boolean;
  /** 已经打印到 text 的第几个字 */
  printed: number;
  /** 最近几次临时结果，用来判断前面的字是否已经稳定 */
  hist: Snap[];
};

const lcp = (a: string, b: string) => {
  let i = 0;
  while (i < a.length && i < b.length && a[i] === b[i]) i++;
  return i;
};

/** 过去 STABLE_MS 里一直没变过的前缀长度 */
function firmLength(u: Utt, now: number): number {
  if (u.final) return u.text.length;
  const old = u.hist.filter((h) => h.t <= now - STABLE_MS);
  if (!old.length) return 0;
  const since = [old[old.length - 1], ...u.hist.filter((h) => h.t > now - STABLE_MS)];
  return Math.min(...since.map((h) => lcp(h.text, u.text)));
}

export function CaptionsTab() {
  const { t } = useI18n();
  const app = useApp();
  const s = app.settings;
  const cap = s.captions;
  const setCap = (patch: Partial<typeof cap>) => app.setSettings({ ...s, captions: { ...cap, ...patch } });

  const [available, setAvailable] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const [locale, setLocale] = useState(() => ({ zh: "zh-CN", ja: "ja-JP", en: "en-US" })[app.lang]);
  const [mic, setMic] = useState(false);
  const [utts, setUtts] = useState<Utt[]>([]);
  const [level, setLevel] = useState<number | null>(null);
  const [printedLines, setPrintedLines] = useState(0);
  const [, setTick] = useState(0);

  const nextId = useRef(1);
  const lastHeard = useRef(0);
  const printing = useRef(false);
  const failedAt = useRef(0);
  const titled = useRef(false);
  const tail = useRef<HTMLDivElement>(null);
  const uttsRef = useRef(utts);
  uttsRef.current = utts;
  const appRef = useRef(app);
  appRef.current = app;
  /** 当前这次收听的编号；旧进程（或界面刷新前）的事件一律忽略 */
  const session = useRef(0);
  const runningRef = useRef(running);
  runningRef.current = running;
  /** 打印机上是否有一个还没结束的连续打印任务 */
  const streaming = useRef(false);

  /** 原始识别文字 → 纸面文字 */
  const toPaper = useCallback(
    (raw: string, n: number, time: string) => {
      const st = appRef.current.settings;
      const text = lint(raw, st.rules).text;
      return text ? applyTemplate(st.template, { text, time, date: formatDate(new Date()), n }) : "";
    },
    [],
  );

  const now = () => formatTime(new Date(), "HH:mm:ss");

  useEffect(() => {
    api.captionsAvailable().then(setAvailable).catch(() => setAvailable("missing"));
    // 界面刷新（比如开发时热更新）后，后台可能还在收听：把状态找回来
    api
      .captionsRunning()
      .then((id) => {
        if (id != null) {
          session.current = id;
          setRunning(true);
        }
      })
      .catch(() => {});
    const mine = (e: { session: number }) => {
      if (e.session < session.current) return false; // 旧进程
      session.current = e.session;
      return true;
    };
    const onText = (raw: string, final: boolean, time?: string) => {
      lastHeard.current = Date.now();
      setUtts((list) => {
        const last = list[list.length - 1];
        const live = last && !last.final ? last : null;
        const n = live ? list.length : list.length + 1;
        const u: Utt = live ?? { id: nextId.current++, time: time ?? now(), text: "", final: false, printed: 0, hist: [] };
        const text = toPaper(raw, n, u.time);
        const t0 = Date.now();
        const next: Utt = {
          ...u,
          text,
          final,
          hist: final ? [] : [...u.hist.filter((h) => h.t > t0 - 3000), { t: t0, text }],
        };
        if (!next.text && final) return live ? list.slice(0, -1) : list;
        return live ? [...list.slice(0, -1), next] : [...list.slice(-400), next];
      });
    };
    const subs = [
      events.volatile((e) => mine(e) && onText(e.text, false)),
      events.caption((e) => mine(e) && onText(e.text, true, e.time)),
      events.level((e) => mine(e) && setLevel(e.rms)),
      events.error((e) => mine(e) && appRef.current.report(e)),
      events.ended((e) => {
        if (e.session !== session.current) return; // 被替换掉的旧进程退出，不影响当前状态
        setRunning(false);
        setLevel(null);
        // 识别器退出：没说完的那句按最终结果处理
        setUtts((list) => list.map((u) => (u.final ? u : { ...u, final: true, hist: [] })));
      }),
    ];
    return () => {
      subs.forEach((p) => p.then((un) => un()));
    };
  }, [toPaper]);

  useEffect(() => {
    tail.current?.scrollIntoView({ block: "end" });
  }, [utts]);

  /** 每句话现在能打印到哪里，以及能打几行 */
  const plan = useCallback((list: Utt[], all: boolean) => {
    const pf = appRef.current.paperFont;
    const px = appRef.current.settings.capSize;
    const t0 = Date.now();
    return list.map((u) => {
      const rest = u.text.slice(u.printed);
      if (!rest.trim()) return { u, upto: u.printed, ready: 0, pending: 0 };
      const lines = paperBreaks(rest, pf, px);
      if (u.final || all) return { u, upto: u.text.length, ready: lines.length, pending: lines.length };
      // 还在说：只打已经排满、并且稳定下来的行（最后一行还在长，不打）
      const firm = firmLength(u, t0) - u.printed;
      let ready = 0;
      let upto = u.printed;
      for (const [, end] of lines.slice(0, -1)) {
        if (end > firm) break;
        ready++;
        upto = u.printed + end;
      }
      return { u, upto, ready, pending: lines.length };
    });
  }, []);

  /** 把能打的部分发给打印机。all = 连没说完的也全打掉（手动、停止时用） */
  const flush = useCallback(
    async (all = false, feed = 0) => {
      if (printing.current) return;
      const a = appRef.current;
      const steps = plan(uttsRef.current, all).filter((p) => p.upto > p.u.printed);
      if (!steps.length) return;
      printing.current = true;
      try {
        const st = a.settings;
        const rows = steps.map(({ u, upto }) => ({
          time: st.timestamp && u.printed === 0 ? (st.timeFormat === "HH:mm" ? u.time.slice(0, 5) : u.time) : undefined,
          text: u.text.slice(u.printed, upto).trim(),
        }));
        const title = st.timestamp && !titled.current ? `${t("cap.title")}  ${formatDate(new Date())}` : undefined;
        const png = tapeCard(rows, a.paperFont, { title, tight: !title, px: st.capSize });
        // 收听中：连续打印，一行接一行不走纸；没在收听：普通打印
        const live = runningRef.current || streaming.current;
        const ok = live ? await a.printLine(png, textDitherOf(a.settings)) : await a.print(png, textDitherOf(a.settings), feed || undefined);
        if (live && ok) streaming.current = true;
        if (ok) {
          titled.current = true;
          const done = new Map(steps.map((p) => [p.u.id, p.upto]));
          setUtts((list) => list.map((u) => (done.has(u.id) ? { ...u, printed: Math.max(u.printed, done.get(u.id)!) } : u)));
          setPrintedLines((n) => n + steps.reduce((k, p) => k + Math.max(1, p.ready), 0));
        } else {
          failedAt.current = Date.now();
        }
      } finally {
        printing.current = false;
      }
    },
    [plan, t],
  );

  // 自动打印的节拍：逐行 = 有完整的行就打；攒行 = 够 N 行或安静 S 秒；手动 = 不自动
  useEffect(() => {
    if (!running) return;
    const timer = window.setInterval(() => {
      setTick((n) => n + 1); // 刷新「还有几行没打」
      if (cap.mode === "manual" || Date.now() - failedAt.current < 15_000) return;
      const p = plan(uttsRef.current, false);
      const ready = p.reduce((k, x) => k + x.ready, 0);
      const quiet = Date.now() - lastHeard.current >= cap.idleSec * 1000;
      if (cap.mode === "line" ? ready > 0 : ready >= cap.batchLines || (quiet && ready > 0)) flush();
    }, 300);
    return () => window.clearInterval(timer);
  }, [running, cap.mode, cap.batchLines, cap.idleSec, plan, flush]);

  const start = async () => {
    titled.current = false;
    try {
      session.current = await api.startCaptions(locale, mic, cap.fast);
      setRunning(true);
    } catch (e) {
      app.report(e);
    }
  };
  const stop = async () => {
    await api.stopCaptions().catch(() => {});
    setRunning(false);
    setLevel(null);
    // 停止时把剩下的打完，再结束连续打印并走纸，方便撕下
    window.setTimeout(async () => {
      if (cap.mode !== "manual" && app.conn === "on") await flush(true);
      if (streaming.current) {
        streaming.current = false;
        await app.endStream();
      }
    }, 400);
  };

  const pendingLines = plan(utts, true).reduce((k, p) => k + p.pending, 0);

  if (available && available !== "ok") {
    return (
      <Machine
        paper={
          <div className="blank left">
            <p>{t(`cap.na.${available}` as Key)}</p>
            {available === "missing" && (
              <>
                <p>{t("cap.build")}</p>
                <code>cd apps/livecap && swift build -c release</code>
              </>
            )}
          </div>
        }
        composer={null}
        action={{ label: t("cap.start"), onPress: () => {}, disabled: true, offline: true }}
      />
    );
  }

  const hint =
    cap.mode === "batch" ? t("cap.hint.batch", { n: cap.batchLines, s: cap.idleSec }) : t(`cap.hint.${cap.mode}` as Key);

  return (
    <Machine
      meter={running ? (level ?? 0) : null}
      paper={
        utts.length === 0 ? (
          <p className="blank">{running ? t("cap.emptyListening") : t("cap.emptyIdle")}</p>
        ) : (
          <PaperScale>
            <Tape utts={utts} />
            <div ref={tail} />
          </PaperScale>
        )
      }
      composer={
        <>
          <div className="row nowrap">
            <Seg
              label={t("cap.system")}
              value={mic ? "mic" : "sys"}
              onChange={(v) => setMic(v === "mic")}
              disabled={running}
              options={[
                ["sys", t("cap.system")],
                ["mic", t("cap.mic")],
              ]}
              small
            />
            <select className="field select" value={locale} disabled={running} onChange={(e) => setLocale(e.target.value)}>
              <option value="zh-CN">中文</option>
              <option value="ja-JP">日本語</option>
              <option value="en-US">English</option>
            </select>
            <button
              className="chip"
              aria-pressed={cap.fast}
              disabled={running}
              title={t("cap.fast.hint")}
              onClick={() => setCap({ fast: !cap.fast })}
            >
              {t("cap.fast")}
            </button>
          </div>
          {cap.fast && <p className="meta warn-line">{t("cap.fast.hint")}</p>}
          <PaperControls sizeKey="capSize" />
          <div className="row nowrap">
            <Seg<CaptionMode>
              label={t("cap.mode")}
              value={cap.mode}
              onChange={(mode) => setCap({ mode })}
              options={(["line", "batch", "manual"] as const).map((m) => [m, t(`cap.mode.${m}` as Key)])}
              small
            />
            {cap.mode === "batch" && (
              <div className="stepper" title={t("cap.batchLines")}>
                <button onClick={() => setCap({ batchLines: Math.max(2, cap.batchLines - 1) })} aria-label="−">
                  −
                </button>
                <output>{cap.batchLines}</output>
                <button onClick={() => setCap({ batchLines: Math.min(30, cap.batchLines + 1) })} aria-label="+">
                  +
                </button>
              </div>
            )}
          </div>
          <p className="meta">
            {hint}
            <br />
            <b>{t("cap.stats", { printed: printedLines, pending: pendingLines })}</b>
            {running && level !== null && level < 0.002 && <span className="warn"> · {t("cap.silent")}</span>}
          </p>
          <div className="row">
            <button className="btn btn-strong" disabled={!pendingLines} onClick={() => flush(true)}>
              {pendingLines ? t("cap.printNowN", { n: pendingLines }) : t("cap.printNow")}
            </button>
            <button
              className="btn btn-quiet"
              disabled={!utts.length}
              onClick={async () => {
                const all = utts.map((u) => `${s.timestamp ? u.time + "  " : ""}${u.text}`).join("\n");
                await navigator.clipboard.writeText(all);
                app.toast(t("common.copied"));
              }}
            >
              {t("common.copy")}
            </button>
            <button
              className="btn btn-quiet"
              disabled={!utts.length}
              onClick={() => {
                setUtts((list) => list.filter((u) => !u.final).map((u) => ({ ...u, printed: u.text.length })));
                setPrintedLines(0);
                titled.current = false;
              }}
            >
              {t("common.clear")}
            </button>
          </div>
        </>
      }
      action={
        running
          ? { label: t("cap.stop"), onPress: stop, stop: true }
          : { label: t("cap.start"), onPress: start, offline: cap.mode === "manual" }
      }
    />
  );
}

/**
 * 字幕纸带预览：和打印用完全相同的折行（paperBreaks）、字体、字号、边距和行距，
 * 在 384 点宽的盒子里排版后整体缩放 —— 屏幕上换行的位置就是纸上换行的位置。
 * 已经打出去的字是黑色，还没打的是灰色。
 */
function Tape({ utts }: { utts: Utt[] }) {
  const app = useApp();
  const s = app.settings;
  const pf = app.paperFont;
  const px = s.capSize;
  // 行高用和打印一样的算法（字体真实高度 vs 字号×行距），预览和纸上一致
  const box = lineBox(pf, px);
  const timeBox = lineBox(pf, 15, 400, 1.25);
  const margin = pf.margin ?? 16;
  return (
    <div className="tape" style={{ fontFamily: pf.family, fontWeight: pf.weight, padding: `2px ${margin}px 6px` }}>
      {utts.map((u, i) => {
        if (!u.text) return null;
        const lines = paperBreaks(u.text, pf, px);
        return (
          <div key={u.id} style={{ marginTop: i ? 8 : 0 }}>
            {s.timestamp && (
              <div className="tape-time" style={{ fontSize: 15, lineHeight: `${timeBox.height}px`, fontWeight: 400 }}>
                {s.timeFormat === "HH:mm" ? u.time.slice(0, 5) : u.time}
              </div>
            )}
            {lines.map(([a, b], k) => {
              const lead = u.text.slice(a, b).length - u.text.slice(a, b).trimStart().length;
              const from = a + lead;
              const cut = Math.min(Math.max(u.printed, from), b);
              return (
                <div key={k} className="tape-line" style={{ fontSize: px, lineHeight: `${box.height}px` }}>
                  <span className="done">{u.text.slice(from, cut)}</span>
                  <span className="todo">{u.text.slice(cut, b).trimEnd()}</span>
                  {k === lines.length - 1 && !u.final && <span className="caret" aria-hidden />}
                </div>
              );
            })}
          </div>
        );
      })}
    </div>
  );
}

