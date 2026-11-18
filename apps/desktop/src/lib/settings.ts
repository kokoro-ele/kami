import type { Lang } from "./i18n";
import type { Rules } from "./format";

export type FontId = "sans" | "serif" | "kai" | "round" | "local";
export type CaptionMode = "line" | "batch" | "manual";

export type AiSettings = {
  provider: "anthropic" | "openai";
  baseUrl: string;
  model: string;
  apiKey: string;
};

export type Settings = {
  lang: "auto" | Lang;
  density: 0 | 1 | 2;
  dither: "fs" | "atkinson" | "threshold";
  font: FontId;
  /** 从本机字体列表里选的字体名 */
  localFont: string;
  weight: 400 | 700;
  /** 文字页、字幕的字号（打印点数，384 点 = 纸宽） */
  textSize: number;
  capSize: number;
  /** 文字转黑白：sharp = 偏深的阈值；detail = 抖动保留灰阶；bold = 再把细线加粗到 2 点（注音、细字体用） */
  textDither: "sharp" | "detail" | "bold";
  /** 行距（字号的倍数） */
  lineHeight: number;
  /** 打印完往前走纸多少毫米（方便撕；越小越省纸） */
  feedMm: 0 | 3 | 5 | 10 | 15;
  /** 实验：告诉打印机用的是连续卷纸，可能消除每段之间的自动走纸 */
  continuousPaper: boolean;
  /** 左右留白（点） */
  margin: 0 | 8 | 16 | 24;
  timestamp: boolean;
  timeFormat: "HH:mm:ss" | "HH:mm";
  template: string;
  rules: Rules;
  ai: AiSettings;
  /** fast：更快出字，但准确率下降 */
  captions: { mode: CaptionMode; batchLines: number; idleSec: number; fast: boolean };
};

export const AI_DEFAULTS: Record<AiSettings["provider"], { baseUrl: string; model: string }> = {
  anthropic: { baseUrl: "https://api.anthropic.com", model: "claude-haiku-5-5" },
  openai: { baseUrl: "https://api.openai.com/v1", model: "gpt-5-mini" },
};

export const DEFAULTS: Settings = {
  lang: "auto",
  density: 1,
  dither: "fs",
  font: "sans",
  localFont: "",
  weight: 700,
  textSize: 28,
  capSize: 24,
  lineHeight: 1.35,
  feedMm: 5,
  continuousPaper: false,
  textDither: "sharp",
  margin: 16,
  timestamp: true,
  timeFormat: "HH:mm:ss",
  template: "{text}",
  rules: { whitespace: true, fillers: false, pangu: true, punctuation: false, custom: "" },
  ai: { provider: "anthropic", ...AI_DEFAULTS.anthropic, apiKey: "" },
  captions: { mode: "line", batchLines: 4, idleSec: 6, fast: false },
};

const FONT_STACKS: Record<Exclude<FontId, "local">, string> = {
  sans: `"PingFang SC", "Hiragino Sans", "Hiragino Sans GB", "Microsoft YaHei", "Yu Gothic", "Noto Sans CJK SC", sans-serif`,
  serif: `"Songti SC", "Hiragino Mincho ProN", "Yu Mincho", "SimSun", "Noto Serif CJK SC", serif`,
  kai: `"Kaiti SC", "STKaiti", "KaiTi", "Klee", serif`,
  round: `"Hiragino Maru Gothic ProN", "Yuanti SC", "Arial Rounded MT Bold", "Microsoft YaHei", sans-serif`,
};

export function fontStack(s: Pick<Settings, "font" | "localFont">): string {
  if (s.font === "local" && s.localFont.trim()) return `"${s.localFont.trim()}", ${FONT_STACKS.sans}`;
  return FONT_STACKS[s.font === "local" ? "sans" : s.font];
}

/** 合并已存的设置和默认值（以后加字段也不会读出 undefined） */
export function loadSettings(): Settings {
  try {
    const raw = JSON.parse(localStorage.getItem("kami.settings") || "{}");
    // 旧版本的取值迁移过来
    if (raw.font === "custom") raw.font = raw.customFont ? "local" : "sans";
    if (raw.customFont && !raw.localFont) raw.localFont = raw.customFont;
    if (raw.captions?.mode === "sentence") raw.captions.mode = "line";
    return {
      ...DEFAULTS,
      ...raw,
      rules: { ...DEFAULTS.rules, ...raw.rules },
      ai: { ...DEFAULTS.ai, ...raw.ai },
      captions: { ...DEFAULTS.captions, ...raw.captions },
    };
  } catch {
    return DEFAULTS;
  }
}

export function saveSettings(s: Settings) {
  try {
    localStorage.setItem("kami.settings", JSON.stringify(s));
  } catch {
    /* 存不了就算了 */
  }
}

export function formatTime(d: Date, fmt: Settings["timeFormat"]): string {
  const p = (n: number) => String(n).padStart(2, "0");
  const hm = `${p(d.getHours())}:${p(d.getMinutes())}`;
  return fmt === "HH:mm" ? hm : `${hm}:${p(d.getSeconds())}`;
}

export function formatDate(d: Date): string {
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}.${p(d.getMonth() + 1)}.${p(d.getDate())}`;
}

/** 套用「每行格式」模板 */
export function applyTemplate(tpl: string, vars: { text: string; time: string; date: string; n: number }): string {
  const t = tpl.includes("{text}") ? tpl : `${tpl}{text}`;
  return t.replace(/\{(text|time|date|n)\}/g, (_, k: keyof typeof vars) => String(vars[k]));
}

/** 文字类打印（许愿、文字、字幕）交给后端的抖动参数 */
export function textDitherOf(s: Pick<Settings, "textDither">): string {
  if (s.textDither === "detail") return "fs";
  if (s.textDither === "bold") return "threshold:190+bold";
  return "threshold:190";
}
