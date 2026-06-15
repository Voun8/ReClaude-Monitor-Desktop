// 监控数据流的单一数据源：当前账号、档案列表、额度快照、各账号状态，
// 以及围绕它们的刷新编排。定时器等生命周期接线留在组件层（MainPanel/MonitorView）。
import {
  api,
  type AccountStatus,
  type EnvInfo,
  type MonitorCred,
  type MonitorSnapshot,
  type ProfileInfo,
} from "./api";
import { toast } from "./ui.svelte";

export const monitor = $state({
  env: null as EnvInfo | null,
  currentEmail: null as string | null,
  profiles: [] as ProfileInfo[],
  cred: null as MonitorCred | null,
  snapshot: null as MonitorSnapshot | null,
  // 非当前账号的额度状态，以 email 为键；loading 与 status 成对维护在同一条目里
  accounts: {} as Record<string, { loading: boolean; status: AccountStatus | null }>,
  refreshing: false,
  lastUpdated: null as number | null,
  // 正在切换中的档案名（驱动列表行 spinner）
  busyName: null as string | null,
});

export function isActiveEmail(email: string): boolean {
  return !!monitor.currentEmail && email.toLowerCase() === monitor.currentEmail.toLowerCase();
}

export function currentProfileName(): string | null {
  if (!monitor.currentEmail) return null;
  const p = monitor.profiles.find(
    (x) => x.email.toLowerCase() === monitor.currentEmail!.toLowerCase(),
  );
  return p ? p.name : null;
}

export async function loadEnv() {
  try {
    monitor.env = await api.getEnv();
  } catch (e) {
    console.error(e);
  }
}

export async function loadProfiles() {
  try {
    monitor.profiles = await api.listProfiles();
  } catch (e) {
    console.error(e);
  }
}

async function loadCurrent(): Promise<boolean> {
  const prev = monitor.currentEmail;
  try {
    monitor.currentEmail = await api.currentAccount();
  } catch (e) {
    console.error(e);
  }
  if (monitor.currentEmail) {
    try {
      monitor.cred = await api.getMonitorCred(monitor.currentEmail);
    } catch {
      monitor.cred = null;
    }
  } else {
    monitor.cred = null;
  }
  return monitor.currentEmail !== prev;
}

// 后端探测到新 orgId（且确实拉到了额度）时写回凭证。
// 同步返回更新后的凭证、后台持久化——调用方对 monitor.cred 的赋值不能延后到
// 网络 await 之后，否则 follow 切换账号期间会被旧凭证踩回。
function syncOrgId(c: MonitorCred, res: { orgId: string; quota: unknown }): MonitorCred {
  if (!res.orgId || res.orgId === c.orgId || !res.quota) return c;
  const updated = { ...c, orgId: res.orgId };
  api.setMonitorCred(updated, null).catch((e) => console.error(e));
  return updated;
}

export async function doRefresh() {
  if (monitor.refreshing) return;
  monitor.refreshing = true;
  try {
    const c = monitor.cred;
    if (!monitor.currentEmail || !c) {
      monitor.snapshot = null;
      return;
    }
    const s = await api.refreshMonitor(c.email, c.password, c.orgId);
    // follow 可能在 await 期间切换了账号：结果属于旧账号则丢弃
    if (monitor.cred?.email !== c.email) return;
    monitor.snapshot = s;
    monitor.lastUpdated = Date.now();
    monitor.cred = syncOrgId(c, s);
  } catch (e) {
    console.error(e);
  } finally {
    monitor.refreshing = false;
  }
}

export async function fetchAccountQuotas() {
  const targets = monitor.profiles.filter((p) => !isActiveEmail(p.email));
  // 限制并发，避免 N 个账号 × 多请求同时撞 API 服务触发限流
  const MAX = 3;
  for (let i = 0; i < targets.length; i += MAX) {
    const batch = targets.slice(i, i + MAX);
    await Promise.all(batch.map(fetchOneAccountQuota));
  }
}

async function fetchOneAccountQuota(p: ProfileInfo) {
  let c: MonitorCred | null = null;
  try {
    c = await api.getMonitorCred(p.email);
  } catch {
    c = null;
  }
  if (!c) return;
  monitor.accounts[p.email] = {
    loading: true,
    status: monitor.accounts[p.email]?.status ?? null,
  };
  try {
    const st = await api.accountStatus(c.email, c.password, c.orgId);
    monitor.accounts[p.email] = { loading: false, status: st };
    syncOrgId(c, st);
  } catch (e) {
    // 写入 error 状态后列表行会显示「获取失败」，不静默吞掉
    monitor.accounts[p.email] = {
      loading: false,
      status: { quota: null, orgId: c.orgId, error: String(e), badCredentials: false },
    };
  }
}

async function refreshData() {
  await Promise.all([doRefresh(), loadProfiles()]);
  await fetchAccountQuotas();
}

export async function refreshAll() {
  await loadCurrent();
  await refreshData();
}

// 跟随外部账号切换（reclaude 侧切了号）：变化时才做整轮刷新
export async function follow() {
  const changed = await loadCurrent();
  if (changed) await refreshData();
}

export async function switchTo(p: ProfileInfo, noApp: boolean) {
  monitor.busyName = p.name;
  try {
    const email = await api.useProfile(p.name, noApp);
    toast(`已切换到 ${email}`);
  } catch (e) {
    toast(String(e), "err");
  } finally {
    monitor.busyName = null;
  }
  await refreshAll();
}
