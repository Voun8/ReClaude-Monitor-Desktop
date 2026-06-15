// 跨组件的瞬态 UI 状态：toast 队列 + 当前弹窗。
import type { ProfileInfo } from "./api";

// ---- toast ----

export type ToastLevel = "ok" | "warn" | "err";

const TOAST_MS = 3200;
let seq = 0;

export const toasts = $state<{ id: number; text: string; level: ToastLevel }[]>([]);

export function toast(text: string, level: ToastLevel = "ok") {
  const id = ++seq;
  toasts.push({ id, text, level });
  setTimeout(() => {
    const i = toasts.findIndex((t) => t.id === id);
    if (i >= 0) toasts.splice(i, 1);
  }, TOAST_MS);
}

// ---- modal ----

export type ModalRequest =
  | { kind: "save" }
  | { kind: "creds"; profileName: string | null; email: string }
  | { kind: "use"; profile: ProfileInfo }
  | { kind: "remove"; profile: ProfileInfo }
  | { kind: "settings" };

export const ui = $state({ modal: null as ModalRequest | null });

export function openModal(m: ModalRequest) {
  ui.modal = m;
}

export function closeModal() {
  ui.modal = null;
}
