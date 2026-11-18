import { getCurrentWindow } from "@tauri-apps/api/window";
import type { MouseEvent } from "react";

/** 标题栏融进了窗口，没有系统标题栏可拖；在空白处按下鼠标就主动开始拖动窗口 */
export function dragWindow(e: MouseEvent) {
  if (e.button !== 0) return;
  if ((e.target as HTMLElement).closest("button, input, select, textarea, a, [role=option]")) return;
  if (!("__TAURI_INTERNALS__" in window)) return;
  e.preventDefault();
  getCurrentWindow()
    .startDragging()
    .catch(() => {});
}
