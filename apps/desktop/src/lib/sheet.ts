// 用浏览器 canvas 排版小票（字体由系统提供，中日英都没问题），
// 输出灰度 PNG，交给 Rust 端抖动 + 打印。

export const WIDTH = 384;

export type PaperFont = { family: string; weight: 400 | 700; margin?: number; lineHeight?: number };

/**
 * 按像素宽度折行，返回每行在原文里的 [起, 止) 位置。
 * 英文按单词断、中日文按字断；遇到 \n 强制换行。字幕逐行打印靠这些位置知道「打到哪了」。
 */
export function breakLines(text: string, measure: (s: string) => number, maxW: number): [number, number][] {
  const out: [number, number][] = [];
  let start = 0;
  let end = 0;
  const flush = (to: number) => {
    out.push([start, to]);
    start = to;
  };
  for (const m of text.matchAll(/\n|[A-Za-z0-9'’@#&%$._\-/:]+[,.!?;:)]?|\s+|./gu)) {
    const tok = m[0];
    const i = m.index!;
    if (tok === "\n") {
      flush(i);
      start = i + 1;
      end = start;
      continue;
    }
    const next = i + tok.length;
    if (measure(text.slice(start, next).trimStart()) <= maxW) {
      end = next;
      continue;
    }
    if (end > start) flush(end);
    if (measure(tok.trim()) <= maxW) {
      end = next;
      continue;
    }
    // 超长单词（网址等）逐字硬断
    for (const ch of Array.from(tok)) {
      if (end > start && measure(text.slice(start, end + ch.length)) > maxW) flush(end);
      end += ch.length;
    }
  }
  if (end > start || out.length === 0) out.push([start, Math.max(end, start)]);
  return out;
}

/** 打开合字和字距调整：像 Furifont 这类靠合字显示注音的字体才能正确显示 */
function enableShaping(ctx: CanvasRenderingContext2D) {
  const c = ctx as CanvasRenderingContext2D & { textRendering?: string; fontKerning?: string };
  try {
    c.textRendering = "optimizeLegibility";
    c.fontKerning = "normal";
  } catch {
    /* 老版本 WebView 不支持就算了 */
  }
}

let measureCtx: CanvasRenderingContext2D | null = null;

function mctx(): CanvasRenderingContext2D {
  if (!measureCtx) {
    measureCtx = document.createElement("canvas").getContext("2d")!;
    enableShaping(measureCtx);
  }
  return measureCtx;
}

/**
 * 一行文字实际要占多高：取「字号 × 行距」和「字体真实的上下高度」里大的那个。
 * 带注音的字体（Furifont）上面多出一截，只按字号算会把字切掉。
 * 返回行高和基线到行顶的距离；打印和屏幕预览都用它，保证两边一致。
 */
export function lineBox(pf: PaperFont, px: number, weight?: number, lh = pf.lineHeight ?? 1.35) {
  const c = mctx();
  c.font = `${weight ?? pf.weight} ${px}px ${pf.family}`;
  const m = c.measureText("国Agあ見");
  const asc = m.fontBoundingBoxAscent || m.actualBoundingBoxAscent || px * 0.88;
  const desc = m.fontBoundingBoxDescent || m.actualBoundingBoxDescent || px * 0.12;
  const height = Math.ceil(Math.max(px * lh, asc + desc));
  const baseline = (height - (asc + desc)) / 2 + asc;
  return { height, baseline };
}

/** 用纸面字体测量折行（不需要真的画出来） */
export function paperBreaks(text: string, pf: PaperFont, px: number): [number, number][] {
  const c = mctx();
  c.font = `${pf.weight} ${px}px ${pf.family}`;
  return breakLines(text, (t) => c.measureText(t).width, WIDTH - 2 * (pf.margin ?? 16));
}

export class Sheet {
  readonly canvas: HTMLCanvasElement;
  readonly ctx: CanvasRenderingContext2D;
  y = 16;
  margin: number;

  constructor(
    private pf: PaperFont,
    maxHeight = 6000,
  ) {
    this.margin = pf.margin ?? 16;
    this.canvas = document.createElement("canvas");
    this.canvas.width = WIDTH;
    this.canvas.height = maxHeight;
    this.ctx = this.canvas.getContext("2d")!;
    enableShaping(this.ctx);
    this.ctx.fillStyle = "#fff";
    this.ctx.fillRect(0, 0, WIDTH, maxHeight);
    this.ctx.textBaseline = "top";
  }

  get inner() {
    return WIDTH - this.margin * 2;
  }

  private font(px: number, weight?: number) {
    this.ctx.font = `${weight ?? this.pf.weight} ${px}px ${this.pf.family}`;
  }

  wrap(text: string, px: number, weight?: number): string[] {
    this.font(px, weight);
    return breakLines(text, (t) => this.ctx.measureText(t).width, this.inner).map(([a, b]) => text.slice(a, b).trim());
  }

  text(text: string, px: number, opts: { align?: "left" | "center"; weight?: number; lh?: number; gray?: boolean } = {}) {
    const { align = "left", weight, lh = this.pf.lineHeight ?? 1.35, gray = false } = opts;
    const box = lineBox(this.pf, px, weight, lh);
    for (const line of this.wrap(text, px, weight)) {
      this.font(px, weight);
      this.ctx.fillStyle = gray ? "#555" : "#000";
      this.ctx.textBaseline = "alphabetic";
      const w = this.ctx.measureText(line).width;
      const x = align === "center" ? (WIDTH - w) / 2 : this.margin;
      this.ctx.fillText(line, x, this.y + box.baseline);
      this.y += box.height;
    }
    return this;
  }

  /** 粗边框里的大字。不用整块黑底：大面积纯黑超出热敏头加热能力，常常打不出来 */
  banner(text: string, px: number) {
    const h = px * 1.45;
    const t = 4;
    this.ctx.fillStyle = "#000";
    this.ctx.fillRect(this.margin, this.y, this.inner, t);
    this.ctx.fillRect(this.margin, this.y + h - t, this.inner, t);
    this.ctx.fillRect(this.margin, this.y, t, h);
    this.ctx.fillRect(WIDTH - this.margin - t, this.y, t, h);
    this.font(px, 700);
    this.ctx.textBaseline = "top";
    const w = this.ctx.measureText(text).width;
    this.ctx.fillText(text, (WIDTH - w) / 2, this.y + (h - px) / 2);
    this.y += h;
    return this;
  }

  rule(t = 3) {
    this.ctx.fillStyle = "#000";
    this.ctx.fillRect(this.margin, this.y, this.inner, t);
    this.y += t;
    return this;
  }

  dashed() {
    this.ctx.fillStyle = "#000";
    for (let x = this.margin; x < WIDTH - this.margin - 4; x += 8) this.ctx.fillRect(x, this.y, 4, 2);
    this.y += 2;
    return this;
  }

  space(dy: number) {
    this.y += dy;
    return this;
  }

  /** 裁到实际高度，返回 PNG data URL */
  finish(bottom = this.margin): string {
    const h = Math.max(1, Math.ceil(this.y + bottom));
    const out = document.createElement("canvas");
    out.width = WIDTH;
    out.height = h;
    out.getContext("2d")!.drawImage(this.canvas, 0, 0);
    return out.toDataURL("image/png");
  }
}

export function textCard(text: string, px: number, center: boolean, pf: PaperFont, stamp?: string): string {
  const s = new Sheet(pf);
  if (stamp) s.text(stamp, 16, { align: center ? "center" : "left", weight: 400, gray: true }).space(4);
  return s.text(text, px, { align: center ? "center" : "left" }).finish();
}

export type TapeLine = { time?: string; text: string };

/** 字幕纸带：可选抬头，每行可选时间戳 */
export function tapeCard(lines: TapeLine[], pf: PaperFont, opts: { title?: string; px?: number; tight?: boolean } = {}): string {
  const s = new Sheet(pf);
  const px = opts.px ?? 24;
  // 连续打印时上下不留白，一段段接起来就是一条完整的纸带，行距和预览一致
  if (opts.tight) s.y = 0;
  if (opts.title) s.text(opts.title, 20, { weight: 700 }).space(2).rule().space(10);
  lines.forEach((l, i) => {
    if (l.time) s.text(l.time, 15, { weight: 400, gray: true, lh: 1.25 });
    s.text(l.text, px);
    if (i < lines.length - 1) s.space(8);
  });
  return s.finish(opts.tight ? 0 : 10);
}

export function fileToDataUrl(file: File): Promise<string> {
  return new Promise((res, rej) => {
    const r = new FileReader();
    r.onload = () => res(r.result as string);
    r.onerror = () => rej(r.error);
    r.readAsDataURL(file);
  });
}
