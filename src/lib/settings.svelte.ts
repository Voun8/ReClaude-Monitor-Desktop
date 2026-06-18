// 设置项的单一数据源：$state 为运行时真相，localStorage 为启动镜像，
// Rust 端 ui.json（saveUiConfig）供下次启动与悬浮球/托盘后台循环读取。
// 读取、校验、范围常量、持久化全部收敛在本文件，页面层不再手写同步。
import { api } from "./api";
import { toast } from "./ui.svelte";

export const REFRESH_SEC_MIN = 5;
export const REFRESH_SEC_MAX = 3600;
const REFRESH_SEC_DEFAULT = 30;
export const FLOAT_SIZE_MIN = 30;
export const FLOAT_SIZE_MAX = 600;
const FLOAT_SIZE_DEFAULT = 160;

export type FloatMode = "ball" | "tray";
export type CloseAction = "quit" | "background";
export type View = "monitor" | "usage";

function lsNum(key: string, def: number, min: number, max: number): number {
  try {
    const v = Number(localStorage.getItem(key));
    if (Number.isFinite(v) && v >= min && v <= max) return v;
  } catch {
    /* localStorage 不可用（隐私模式等），用默认值 */
  }
  return def;
}

function lsStr(key: string, def: string): string {
  try {
    const v = localStorage.getItem(key);
    if (v) return v;
  } catch {
    /* 同上 */
  }
  return def;
}

function lsSet(key: string, value: string | null) {
  try {
    if (value === null) localStorage.removeItem(key);
    else localStorage.setItem(key, value);
  } catch {
    /* 同上 */
  }
}

export const settings = $state({
  refreshSec: lsNum("refreshSec", REFRESH_SEC_DEFAULT, REFRESH_SEC_MIN, REFRESH_SEC_MAX),
  // 最小化方式：悬浮球 或 菜单栏圆环
  floatMode: (lsStr("floatMode", "ball") === "tray" ? "tray" : "ball") as FloatMode,
  floatSize: lsNum("floatSize", FLOAT_SIZE_DEFAULT, FLOAT_SIZE_MIN, FLOAT_SIZE_MAX),
  apiBase: lsStr("apiBase", ""),
  // 关闭按钮行为：退出程序 或 后台运行
  closeAction: (lsStr("closeAction", "quit") === "background" ? "background" : "quit") as CloseAction,
  view: (lsStr("view", "monitor") === "usage" ? "usage" : "monitor") as View,
  // 切换账号时只换凭证、不自动打开桌面 App
  noApp: lsStr("noApp", "1") === "1",
  // 静默启动：启动直接进后台指示器（圆环/悬浮球）不弹主窗口（localStorage 镜像，默认开，与后端 silent.unwrap_or(true) 一致）
  silentStart: lsStr("silentStart", "1") === "1",
  // 开机自启动：状态由系统持有，initFromBackend 查询刷新（不进 localStorage，避免与系统真值漂移）
  autostart: false,
});

export function setView(v: View) {
  settings.view = v;
  lsSet("view", v);
}

export function setNoApp(v: boolean) {
  settings.noApp = v;
  lsSet("noApp", v ? "1" : "0");
}

export function normalizeApiBaseInput(raw: string): string | null {
  const trimmed = raw.trim();
  if (!trimmed) return "";
  const withScheme = /^https?:\/\//i.test(trimmed) ? trimmed : `https://${trimmed}`;
  try {
    return new URL(withScheme).origin.replace(/\/+$/, "");
  } catch {
    return null;
  }
}

export interface SettingsDraft {
  refreshSec: number;
  floatMode: FloatMode;
  floatSize: number;
  apiBase: string;
  closeAction: CloseAction;
  silentStart: boolean;
  autostart: boolean;
}

export function saveSettings(
  draft: SettingsDraft,
): { ok: true; apiChanged: boolean } | { ok: false; error: string } {
  const sec = Math.round(draft.refreshSec);
  const size = Math.round(draft.floatSize);
  const nextApiBase = normalizeApiBaseInput(draft.apiBase);
  if (!Number.isFinite(sec) || sec < REFRESH_SEC_MIN || sec > REFRESH_SEC_MAX) {
    return { ok: false, error: `刷新间隔需在 ${REFRESH_SEC_MIN}–${REFRESH_SEC_MAX} 秒之间` };
  }
  if (!Number.isFinite(size) || size < FLOAT_SIZE_MIN || size > FLOAT_SIZE_MAX) {
    return { ok: false, error: `悬浮球大小需在 ${FLOAT_SIZE_MIN}–${FLOAT_SIZE_MAX} 之间` };
  }
  if (nextApiBase === null) {
    return { ok: false, error: "API 地址格式不正确" };
  }
  const apiChanged = settings.apiBase !== nextApiBase;
  settings.refreshSec = sec;
  settings.floatMode = draft.floatMode;
  settings.floatSize = size;
  settings.apiBase = nextApiBase;
  settings.closeAction = draft.closeAction;
  settings.silentStart = draft.silentStart;
  lsSet("refreshSec", String(sec));
  lsSet("floatMode", settings.floatMode);
  lsSet("floatSize", String(size));
  lsSet("apiBase", nextApiBase || null);
  lsSet("closeAction", settings.closeAction);
  lsSet("silentStart", settings.silentStart ? "1" : "0");
  // 开机自启动：系统真值，变了才写；乐观更新 UI，失败回滚并提示
  if (draft.autostart !== settings.autostart) {
    const want = draft.autostart;
    const prev = settings.autostart;
    settings.autostart = want;
    api.setAutostart(want).catch((e) => {
      settings.autostart = prev;
      toast(`开机自启设置失败：${e}`, "err");
    });
  }
  // 若悬浮球当前可见，立即按新尺寸调整
  api.resizeFloat(size).catch((e) => console.error(e));
  persistUiConfig();
  applyTrayMode();
  return { ok: true, apiChanged };
}

// 持久化到 ui.json 供下次启动时 Rust 读取（refreshSec 同步给悬浮球，silent 决定下次启动是否弹主窗口）
export function persistUiConfig() {
  api
    .saveUiConfig(settings.floatMode, settings.floatSize, settings.refreshSec, settings.apiBase, settings.silentStart)
    .catch((e) => console.error(e));
}

// 启动时以 Rust 端 ui.json 为准同步 apiBase（localStorage 仅作镜像）、查询系统开机自启状态，
// 并把当前设置写回供悬浮球/下次启动读取。
export async function initFromBackend() {
  try {
    settings.apiBase = await api.getApiBase();
    lsSet("apiBase", settings.apiBase || null);
    settings.autostart = await api.getAutostart();
    await api.saveUiConfig(settings.floatMode, settings.floatSize, settings.refreshSec, settings.apiBase, settings.silentStart);
  } catch (e) {
    console.error(e);
  }
}

// 同步菜单栏圆环开关：圆环渲染/刷新全在 Rust 后台循环里完成，前端只切开关 + 传刷新间隔
export async function applyTrayMode() {
  try {
    if (settings.floatMode === "tray") {
      await api.hideFloat(); // 与悬浮球互斥
      await api.setTrayMode(true, settings.refreshSec);
    } else {
      await api.setTrayMode(false, settings.refreshSec);
    }
  } catch (e) {
    console.error(e);
  }
}

// 最小化：按模式 → 悬浮球 或 菜单栏圆环（托盘常驻，仅最小化主窗口）
export async function enterFloat() {
  try {
    if (settings.floatMode === "tray") {
      await api.hideMain();
    } else {
      await api.minimizeToFloat(settings.floatSize);
    }
  } catch (e) {
    toast(String(e), "err");
  }
}
