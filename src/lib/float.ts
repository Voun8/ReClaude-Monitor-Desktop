// 悬浮球的无状态辅助：透明窗口副作用 + 纯数据拉取链。
// 注意：status 归一化、竞态守卫、snapshot 回写等有状态/跨进程逻辑刻意留在 FloatWidget 自身，
// 不在此文件——悬浮球与主窗口各自维护一份，刻意不共享。
import { api, type MonitorSnapshot } from "./api";
import { getCurrentWindow } from "@tauri-apps/api/window";

// 悬浮球纯拉取链的结果：
// - nocred：无当前账号或无凭证（对应原 status="nocred"）
// - ok：成功拿到快照（含 badCredentials 情形，由调用方判读 snap.badCredentials）
// 拉取过程抛出的异常不在此吞掉，交由调用方 catch（保留其有状态回退）。
export type FloatFetchResult =
  | { kind: "nocred" }
  | { kind: "ok"; snapshot: MonitorSnapshot };

// 纯拉取链：current account → monitor cred → refresh。
// 仅负责拉取，不含 status 状态机、竞态守卫与 snapshot 回写。
export async function fetchFloatSnapshot(): Promise<FloatFetchResult> {
  const email = await api.currentAccount();
  if (!email) return { kind: "nocred" };
  const c = await api.getMonitorCred(email);
  if (!c) return { kind: "nocred" };
  const snapshot = await api.refreshMonitor(c.email, c.password, c.orgId);
  return { kind: "ok", snapshot };
}

// 读取刷新间隔（毫秒）：跟随主面板写到 ui.json 的 refreshSec；缺失或越界回落 30s。
export async function getFloatIntervalMs(): Promise<number> {
  let intervalMs = 30_000;
  try {
    const sec = await api.getRefreshSec();
    if (sec && sec >= 5 && sec <= 3600) intervalMs = sec * 1000;
  } catch {
    /* ignore */
  }
  return intervalMs;
}

// 悬浮窗专属窗口副作用：html/body 透明且不滚动（用 JS 设，避免全局 CSS 污染主窗口滚动），
// 并把关闭请求（如 Alt+F4）改为退出整个程序。返回 cleanup 注销监听。
export async function setupFloatWindow(): Promise<() => void> {
  for (const el of [document.documentElement, document.body]) {
    el.style.setProperty("background", "transparent", "important");
    el.style.setProperty("overflow", "hidden", "important");
  }

  // 关闭悬浮球窗口（如 Alt+F4）= 退出整个程序；否则只销毁球、主窗口仍隐藏，反而更难退出。
  // 日常退出走系统托盘图标：右键 →「退出程序」。
  let unlistenClose: (() => void) | undefined;
  try {
    unlistenClose = await getCurrentWindow().onCloseRequested((event) => {
      event.preventDefault();
      api.quitApp();
    });
  } catch {
    /* ignore */
  }

  return () => unlistenClose?.();
}
