import { invoke } from "@tauri-apps/api/core";

export interface EnvInfo {
  reclaudeFound: boolean;
  reclaudePath: string;
  profilesDir: string;
  appdataDir: string;
  currentEmail: string | null;
}

export interface ProfileInfo {
  name: string;
  email: string;
  hasAppSession: boolean;
  hasMonitor: boolean;
}

export interface MonitorCred {
  email: string;
  password: string;
  orgId: string;
}

export interface QuotaOut {
  usedUsd: number;
  totalUsd: number;
  remainingUsd: number;
  pct: number;
  ratio: number;
  resetAtMs: number;
  enabled: boolean;
}

export interface MetricsOut {
  errorRatePct: number;
  errorCount: number;
  reqCount: number;
  avgLatencyMs: number;
  rpm: number;
  tpm: number;
  stateLevel: "ok" | "warn" | "err";
  stateText: string;
}

export interface MonitorSnapshot {
  quota: QuotaOut | null;
  // 后端 account_status 不含 metrics（skip_serializing_if）→ 该字段可能缺省
  metrics?: MetricsOut | null;
  orgId: string;
  error: string | null;
  badCredentials: boolean;
}

// 单账号状态 = 快照去掉 metrics（后端 account_status 不拉服务指标）
export type AccountStatus = Omit<MonitorSnapshot, "metrics">;

export interface Allocation {
  id: string;
  label: string;
  capacity: number | null;
}

export interface UsageOverview {
  sessions: number;
  messages: number;
  totalUsd: number;
  totalTokens: number;
  activeDays: number;
  currentStreak: number;
  longestStreak: number;
  favoriteModel: string;
}

export interface HeatCell {
  date: string;
  count: number;
}

export interface ModelUsage {
  model: string;
  totalUsd: number;
  totalTokens: number;
  percent: number;
}

export interface UsageStats {
  overview: UsageOverview;
  heatmap: HeatCell[];
  models: ModelUsage[];
}

export interface Device {
  id: string;
  name: string;
}

export const api = {
  getEnv: () => invoke<EnvInfo>("get_env"),
  currentAccount: () => invoke<string | null>("current_account"),
  listProfiles: () => invoke<ProfileInfo[]>("list_profiles"),
  getMonitorCred: (email: string) =>
    invoke<MonitorCred | null>("get_monitor_cred", { email }),
  setMonitorCred: (cred: MonitorCred, profileName: string | null) =>
    invoke<void>("set_monitor_cred", { cred, profileName }),
  saveProfile: (name: string, monitor: MonitorCred | null) =>
    invoke<string>("save_profile", { name, monitor }),
  useProfile: (name: string, noApp: boolean) =>
    invoke<string>("use_profile", { name, noApp }),
  removeProfile: (name: string) => invoke<void>("remove_profile", { name }),
  refreshMonitor: (email: string, password: string, orgId: string) =>
    invoke<MonitorSnapshot>("refresh_monitor", { email, password, orgId }),
  accountStatus: (email: string, password: string, orgId: string) =>
    invoke<AccountStatus>("account_status", { email, password, orgId }),
  listAllocations: (email: string, password: string) =>
    invoke<Allocation[]>("list_allocations", { email, password }),
  usageDevices: (email: string, password: string) =>
    invoke<Device[]>("usage_devices", { email, password }),
  usageSync: (email: string, password: string) =>
    invoke<void>("usage_sync", { email, password }),
  usageStats: (
    email: string,
    password: string,
    range: string,
    deviceId: string | null,
    orgId: string,
  ) =>
    invoke<UsageStats>("usage_stats", { email, password, range, deviceId, orgId }),
  hideFloat: () => invoke<void>("hide_float"),
  minimizeToFloat: (size: number) =>
    invoke<void>("minimize_to_float", { size }),
  restoreFromFloat: () => invoke<void>("restore_from_float"),
  resizeFloat: (size: number) => invoke<void>("resize_float", { size }),
  hideMain: () => invoke<void>("hide_main"),
  quitApp: () => invoke<void>("quit_app"),
  // 托盘面板 →「打开主面板」(settings=true 时同时弹设置弹窗)
  openMain: (settings: boolean) => invoke<void>("open_main", { settings }),
  // 主窗口读取并清除「待打开设置」标志
  takePendingSettings: () => invoke<boolean>("take_pending_settings"),
  saveUiConfig: (mode: string, size: number, refreshSec?: number, apiBase?: string, silent?: boolean, apiKey?: string) =>
    invoke<void>("save_ui_config", {
      mode,
      size,
      refreshSec: refreshSec ?? null,
      apiBase: apiBase ?? null,
      silent: silent ?? null,
      apiKey: apiKey ?? null,
    }),
  getRefreshSec: () => invoke<number | null>("get_refresh_sec"),
  getApiBase: () => invoke<string>("get_api_base"),
  getApiKey: () => invoke<string>("get_api_key"),
  setTrayMode: (active: boolean, interval: number) =>
    invoke<void>("set_tray_mode", { active, interval }),
  // 开机自启动：状态由系统持有，get 查询、set 启用/禁用
  getAutostart: () => invoke<boolean>("get_autostart"),
  setAutostart: (enable: boolean) => invoke<void>("set_autostart", { enable }),
};
