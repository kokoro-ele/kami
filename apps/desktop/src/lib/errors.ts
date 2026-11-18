import { hasKey, type T } from "./i18n";

export type ErrInfo = { code: string; detail: string };

/** 每种错误可以提供的修复操作 */
export type FixAction = "connect" | "screen" | "mic" | "bluetooth" | "settings";

const ACTIONS: Record<string, FixAction> = {
  printer_not_found: "connect",
  printer_disconnected: "connect",
  bluetooth_unavailable: "bluetooth",
  ai_auth: "settings",
  ai_unreachable: "settings",
  ai_failed: "settings",
  font_missing: "settings",
  screen_permission: "screen",
  no_audio: "screen",
  mic_permission: "mic",
  no_microphone: "mic",
};

/** 后端返回的是 JSON 字符串 {"code","detail"}；其他情况一律当作 unknown */
export function parseErr(e: unknown): ErrInfo {
  if (e && typeof e === "object" && "code" in e) {
    const o = e as { code: string; detail?: string };
    return { code: o.code, detail: o.detail ?? "" };
  }
  const s = typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e);
  try {
    const o = JSON.parse(s);
    if (o && typeof o.code === "string") return { code: o.code, detail: o.detail ?? "" };
  } catch {
    /* 不是 JSON */
  }
  return { code: "unknown", detail: s };
}

export function describe(err: ErrInfo, t: T) {
  const code = hasKey(`err.${err.code}.title`) ? err.code : "unknown";
  return {
    title: t(`err.${code}.title` as never),
    hint: t(`err.${code}.hint` as never),
    action: ACTIONS[code] as FixAction | undefined,
    detail: err.detail,
  };
}
