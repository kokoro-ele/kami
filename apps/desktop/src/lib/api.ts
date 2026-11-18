import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AiSettings } from "./settings";
import type { ErrInfo } from "./errors";

export type Status = { connected: boolean; name: string | null };
export type Entry = { time: string; text: string };
export type FontInfo = { family: string; alias: string | null; cjk: boolean };

export const api = {
  connect: (prefix?: string) => invoke<Status>("connect", { prefix: prefix || null }),
  status: () => invoke<Status>("status"),
  disconnect: () => invoke<void>("disconnect"),
  previewPng: (png: string, dither: string) => invoke<string>("preview_png", { png, dither }),
  printPng: (png: string, density: number, dither: string, feed?: number) =>
    invoke<void>("print_png", { png, density, dither, feed: feed ?? null }),
  listFonts: () => invoke<FontInfo[]>("list_fonts"),
  /** 字体文件原始字节（给 FontFace 用） */
  fontFile: (family: string) => invoke<ArrayBuffer>("font_file", { family }),
  /** 连续打印一段（字幕逐行），行与行之间不走纸 */
  printLine: (png: string, density: number, dither: string) => invoke<void>("print_line", { png, density, dither }),
  /** 结束连续打印并走纸 */
  endStream: (density: number, feed: number) => invoke<void>("end_stream", { density, feed }),
  setPaperMode: (mode: number | null) => invoke<void>("set_paper_mode", { mode }),
  aiComplete: (ai: AiSettings, prompt: string) =>
    invoke<string>("ai_complete", {
      req: { provider: ai.provider, base_url: ai.baseUrl, model: ai.model, api_key: ai.apiKey, prompt },
    }),
  openSystemSettings: (pane: "screen" | "mic" | "bluetooth") => invoke<void>("open_system_settings", { pane }),
  captionsAvailable: () => invoke<"ok" | "platform" | "os" | "missing" | "unsupported">("captions_available"),
  startCaptions: (locale: string, mic: boolean, fast: boolean) => invoke<number>("start_captions", { locale, mic, fast }),
  /** 后台是否还在收听（界面刷新后找回状态），返回当前 session */
  captionsRunning: () => invoke<number | null>("captions_running"),
  stopCaptions: () => invoke<void>("stop_captions"),
};

const on =
  <T,>(name: string) =>
  (cb: (v: T) => void): Promise<UnlistenFn> =>
    listen<T>(name, (ev) => cb(ev.payload));

/** 每个事件都带 session：同一时间只处理当前这次收听的事件 */
type S<T> = T & { session: number };
export const events = {
  caption: on<S<Entry>>("caption"),
  volatile: on<S<{ text: string }>>("caption-volatile"),
  level: on<S<{ rms: number }>>("caption-level"),
  error: on<S<ErrInfo>>("captions-error"),
  ended: on<S<object>>("captions-ended"),
};
