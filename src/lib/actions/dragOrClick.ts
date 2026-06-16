import type { Action } from "svelte/action";
import { getCurrentWindow } from "@tauri-apps/api/window";

// 手动区分拖拽与点击的 action（悬浮球用）。
// 不用 data-tauri-drag-region——它在 Windows 上 pointerdown 即进系统拖拽循环，
// 会吞掉 click 事件，导致「点好几次才打开主面板」。改为：移动超阈值才 startDragging，
// 未移动的 pointerup 判定为点击 → 回调 onClick（由调用方决定点击行为）。
//
// 注意：这里只封装纯交互判定（指针捕获、阈值、startDragging），不持有任何业务/跨进程状态。

export interface DragOrClickOptions {
  // 未触发拖拽的 pointerup 判定为点击时回调
  onClick: () => void;
  // 移动阈值（px）；超过即进入系统拖拽。默认 8：点击时的轻微抖动不被误判为拖拽。
  threshold?: number;
}

export const dragOrClick: Action<HTMLElement, DragOrClickOptions> = (
  node,
  options,
) => {
  const appWindow = getCurrentWindow();
  let opts = options;
  let downX = 0;
  let downY = 0;
  let pointerDown = false;
  let dragging = false;

  function onDown(e: PointerEvent) {
    if (e.button !== 0) return;
    pointerDown = true;
    dragging = false;
    downX = e.clientX;
    downY = e.clientY;
    // 捕获指针：之后的 move/up 一律派发到本元素，避免指针掠过动画水波层或
    // pointer-events:none 的透明环时 pointerup 落在别处 → 这次点击丢失（上半圆尤其常见）。
    try {
      node.setPointerCapture(e.pointerId);
    } catch {
      /* ignore */
    }
  }

  async function onMove(e: PointerEvent) {
    if (!pointerDown || dragging) return;
    const threshold = opts.threshold ?? 8;
    // 阈值放宽到 8px：点击时的轻微抖动不再被误判为拖拽（否则会触发 startDragging 把这次点击吃掉）。
    if (Math.hypot(e.clientX - downX, e.clientY - downY) > threshold) {
      dragging = true;
      try {
        await appWindow.startDragging();
      } catch {
        /* ignore */
      }
    }
  }

  function onUp(e: PointerEvent) {
    try {
      node.releasePointerCapture(e.pointerId);
    } catch {
      /* ignore */
    }
    if (pointerDown && !dragging) opts.onClick();
    pointerDown = false;
  }

  node.addEventListener("pointerdown", onDown);
  node.addEventListener("pointermove", onMove);
  node.addEventListener("pointerup", onUp);

  return {
    update(next: DragOrClickOptions) {
      opts = next;
    },
    destroy() {
      node.removeEventListener("pointerdown", onDown);
      node.removeEventListener("pointermove", onMove);
      node.removeEventListener("pointerup", onUp);
    },
  };
};
