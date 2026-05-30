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
  metrics: MetricsOut | null;
  orgId: string;
  error: string | null;
  badCredentials: boolean;
}

export interface AccountStatus {
  quota: QuotaOut | null;
  orgId: string;
  error: string | null;
  badCredentials: boolean;
}

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
  toggleFloat: () => invoke<boolean>("toggle_float"),
  hideFloat: () => invoke<void>("hide_float"),
  showMain: () => invoke<void>("show_main"),
};
