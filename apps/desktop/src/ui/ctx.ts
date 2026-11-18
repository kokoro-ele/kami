import { createContext, useContext } from "react";
import type { Lang } from "../lib/i18n";
import type { Settings } from "../lib/settings";
import type { PaperFont } from "../lib/sheet";

export type Conn = "off" | "connecting" | "on";

export type AppCtx = {
  conn: Conn;
  printerName: string | null;
  lang: Lang;
  settings: Settings;
  setSettings: (s: Settings) => void;
  paperFont: PaperFont;
  connect: () => Promise<void>;
  disconnect: () => Promise<void>;
  /** 打印一张图；失败时自动显示错误提示并返回 false */
  print: (png: string, dither: string, feed?: number) => Promise<boolean>;
  /** 字幕连续打印：一行一行接着打，中间不走纸 */
  printLine: (png: string, dither: string) => Promise<boolean>;
  /** 结束连续打印并走纸 */
  endStream: (feed?: number) => Promise<void>;
  report: (e: unknown) => void;
  toast: (msg: string) => void;
  openSettings: () => void;
};

export const Ctx = createContext<AppCtx>(null as unknown as AppCtx);
export const useApp = () => useContext(Ctx);
