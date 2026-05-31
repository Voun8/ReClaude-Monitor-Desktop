<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import {
    api,
    type AccountStatus,
    type Allocation,
    type EnvInfo,
    type MonitorCred,
    type MonitorSnapshot,
    type ProfileInfo,
  } from "$lib/api";
  import { fmtCountdown, fmtMs, fmtUsd, quotaColor } from "$lib/format";
  import Gauge from "$lib/components/Gauge.svelte";
  import AccountRow from "$lib/components/AccountRow.svelte";
  import Modal from "$lib/components/Modal.svelte";
  import UsageView from "$lib/components/UsageView.svelte";
  import FloatWidget from "$lib/components/FloatWidget.svelte";
  import Logo from "$lib/components/Logo.svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import {
    RefreshCw,
    Plus,
    KeyRound,
    Clock,
    Wallet,
    AlertTriangle,
    LogIn,
    PictureInPicture2,
    Settings,
  } from "@lucide/svelte";

  function detectFloat(): boolean {
    try {
      return getCurrentWindow().label === "float";
    } catch {
      return false;
    }
  }
  const isFloat = detectFloat();

  const FOLLOW_MS = 10_000;

  // 设置项（持久化到 localStorage）
  function lsNum(key: string, def: number, min: number, max: number): number {
    try {
      const v = Number(localStorage.getItem(key));
      if (Number.isFinite(v) && v >= min && v <= max) return v;
    } catch {
      /* ignore */
    }
    return def;
  }
  function lsStr(key: string, def: string): string {
    try {
      const v = localStorage.getItem(key);
      if (v) return v;
    } catch {
      /* ignore */
    }
    return def;
  }
  let refreshSec = $state(lsNum("refreshSec", 30, 5, 3600));
  // 最小化方式：悬浮球 或 菜单栏圆环
  let floatMode = $state<"ball" | "tray">(
    lsStr("floatMode", "ball") === "tray" ? "tray" : "ball",
  );
  let floatSize = $state(lsNum("floatSize", 160, 30, 600));
  // 关闭按钮行为：退出程序 或 后台运行
  let closeAction = $state<"quit" | "background">(
    lsStr("closeAction", "quit") === "background" ? "background" : "quit",
  );
  let usageReloadKey = $state(0); // 递增以触发用量页重新加载

  let env = $state<EnvInfo | null>(null);
  let currentEmail = $state<string | null>(null);
  let profiles = $state<ProfileInfo[]>([]);
  let snapshot = $state<MonitorSnapshot | null>(null);
  let cred = $state<MonitorCred | null>(null);
  let accountStatuses = $state<Record<string, AccountStatus>>({});
  let accountLoading = $state<Record<string, boolean>>({});
  let now = $state(Date.now());
  let busyName = $state<string | null>(null);
  let refreshing = $state(false);
  let noApp = $state(true);
  let lastUpdated = $state<number | null>(null);
  let view = $state<"monitor" | "usage">(
    lsStr("view", "monitor") === "usage" ? "usage" : "monitor",
  );

  let toasts = $state<{ id: number; text: string; level: string }[]>([]);
  let toastSeq = 0;

  type ModalState =
    | null
    | { kind: "save"; name: string; incMon: boolean; email: string; password: string; orgId: string; busy: boolean }
    | { kind: "creds"; profileName: string | null; email: string; password: string; orgId: string; allocs: Allocation[] | null; detecting: boolean; busy: boolean }
    | { kind: "use"; profile: ProfileInfo }
    | { kind: "remove"; profile: ProfileInfo; busy: boolean }
    | {
        kind: "settings";
        sec: number;
        mode: "ball" | "tray";
        size: number;
        closeAct: "quit" | "background";
      };
  let modal = $state<ModalState>(null);

  // ---- derived ----
  const currentName = $derived.by(() => {
    if (!currentEmail) return null;
    const p = profiles.find(
      (x) => x.email.toLowerCase() === currentEmail!.toLowerCase(),
    );
    return p ? p.name : null;
  });

  const heroBadge = $derived.by(() => {
    if (!cred) return null;
    const q = snapshot?.quota;
    const m = snapshot?.metrics;
    if (snapshot?.badCredentials) return { level: "err", text: "密码错误" };
    if (q && q.ratio >= 0.95) return { level: "err", text: "额度告急" };
    if (m && m.stateLevel === "err") return { level: "err", text: "服务故障" };
    if (m && m.stateLevel === "warn") return { level: "warn", text: "服务抖动" };
    if (q && q.ratio >= 0.8) return { level: "warn", text: "额度偏高" };
    if (m || q) return { level: "ok", text: "正常" };
    return null;
  });

  const resetText = $derived(
    snapshot?.quota?.resetAtMs ? fmtCountdown(snapshot.quota.resetAtMs - now) : "",
  );

  const heroMessage = $derived.by(() => {
    if (!cred) return "配置监控凭证后即可查看额度";
    if (snapshot?.badCredentials) return "账号或密码错误，请更新凭证";
    if (snapshot && !snapshot.orgId) return "该账号下没有拼车套餐";
    if (snapshot?.error) return "额度获取失败，稍后重试";
    return "加载中…";
  });

  function isActiveEmail(email: string): boolean {
    return !!currentEmail && email.toLowerCase() === currentEmail.toLowerCase();
  }

  function quotaFor(p: ProfileInfo) {
    if (isActiveEmail(p.email)) return snapshot?.quota ?? null;
    return accountStatuses[p.email]?.quota ?? null;
  }
  function loadingFor(p: ProfileInfo): boolean {
    if (isActiveEmail(p.email)) return false;
    return accountLoading[p.email] === true;
  }
  function errTextFor(p: ProfileInfo): string | null {
    if (isActiveEmail(p.email)) return null;
    const st = accountStatuses[p.email];
    if (!st) return null;
    if (st.badCredentials) return "密码错误";
    if (!st.quota && !st.orgId) return "无拼车套餐";
    if (st.error && !st.quota) return "获取失败";
    return null;
  }

  // ---- helpers ----
  function toast(text: string, level = "ok") {
    const id = ++toastSeq;
    toasts.push({ id, text, level });
    setTimeout(() => {
      toasts = toasts.filter((t) => t.id !== id);
    }, 3200);
  }

  // ---- data flow ----
  async function loadProfiles() {
    try {
      profiles = await api.listProfiles();
    } catch (e) {
      console.error(e);
    }
  }

  async function loadCurrent(): Promise<boolean> {
    const prev = currentEmail;
    try {
      currentEmail = await api.currentAccount();
    } catch (e) {
      console.error(e);
    }
    if (currentEmail) {
      try {
        cred = await api.getMonitorCred(currentEmail);
      } catch {
        cred = null;
      }
    } else {
      cred = null;
    }
    return currentEmail !== prev;
  }

  async function doRefresh() {
    if (refreshing) return;
    refreshing = true;
    try {
      if (!currentEmail || !cred) {
        snapshot = null;
        return;
      }
      const s = await api.refreshMonitor(cred.email, cred.password, cred.orgId);
      snapshot = s;
      lastUpdated = Date.now();
      if (s.orgId && s.orgId !== cred.orgId && s.quota) {
        const updated = { ...cred, orgId: s.orgId };
        cred = updated;
        try {
          await api.setMonitorCred(updated, null);
        } catch {
          /* ignore */
        }
      }
    } catch (e) {
      console.error(e);
    } finally {
      refreshing = false;
    }
  }

  async function fetchAccountQuotas() {
    const targets = profiles.filter((p) => !isActiveEmail(p.email));
    // 限制并发，避免 N 个账号 × 多请求同时撞 reclaude.ai 触发限流
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
    accountLoading[p.email] = true;
    try {
      const st = await api.accountStatus(c.email, c.password, c.orgId);
      accountStatuses[p.email] = st;
      if (st.orgId && st.orgId !== c.orgId && st.quota) {
        try {
          await api.setMonitorCred({ ...c, orgId: st.orgId }, null);
        } catch {
          /* ignore */
        }
      }
    } catch (e) {
      // 之前是静默吞掉，用户看不到原因；写入 error 状态后 errTextFor 会显示"获取失败"
      accountStatuses[p.email] = {
        quota: null,
        orgId: c.orgId,
        error: String(e),
        badCredentials: false,
      };
    } finally {
      accountLoading[p.email] = false;
    }
  }

  async function refreshAll() {
    await loadCurrent();
    await Promise.all([doRefresh(), loadProfiles()]);
    await fetchAccountQuotas();
  }

  async function follow() {
    const changed = await loadCurrent();
    if (changed) {
      await Promise.all([doRefresh(), loadProfiles()]);
      await fetchAccountQuotas();
    }
  }

  // ---- profile actions ----
  function openSave() {
    modal = {
      kind: "save",
      name: currentName ?? "",
      incMon: !cred,
      email: currentEmail ?? "",
      password: cred?.password ?? "",
      orgId: cred?.orgId ?? "",
      busy: false,
    };
  }

  async function confirmSave() {
    if (modal?.kind !== "save") return;
    const name = modal.name.trim();
    if (!name) {
      toast("请填写档案名", "err");
      return;
    }
    modal.busy = true;
    const mon: MonitorCred | null =
      modal.incMon && modal.email.trim() && modal.password
        ? { email: modal.email.trim(), password: modal.password, orgId: modal.orgId.trim() }
        : null;
    try {
      const email = await api.saveProfile(name, mon);
      toast(`已保存档案 “${name}”`);
      modal = null;
      await refreshAll();
      void email;
    } catch (e) {
      toast(String(e), "err");
      if (modal?.kind === "save") modal.busy = false;
    }
  }

  function openUse(p: ProfileInfo) {
    modal = { kind: "use", profile: p };
  }

  async function confirmUse() {
    if (modal?.kind !== "use") return;
    const p = modal.profile;
    modal = null;
    busyName = p.name;
    try {
      const email = await api.useProfile(p.name, noApp);
      toast(`已切换到 ${email}`);
    } catch (e) {
      toast(String(e), "err");
    } finally {
      busyName = null;
    }
    await refreshAll();
  }

  function openRemove(p: ProfileInfo) {
    modal = { kind: "remove", profile: p, busy: false };
  }

  async function confirmRemove() {
    if (modal?.kind !== "remove") return;
    const p = modal.profile;
    modal.busy = true;
    try {
      await api.removeProfile(p.name);
      // 清理该 email 的缓存状态，避免删除后 accountStatuses/Loading 留下孤儿键
      delete accountStatuses[p.email];
      delete accountLoading[p.email];
      toast(`已删除档案 “${p.name}”`);
      modal = null;
      await loadProfiles();
    } catch (e) {
      toast(String(e), "err");
      if (modal?.kind === "remove") modal.busy = false;
    }
  }

  // ---- monitor credentials ----
  async function openCreds(profileName: string | null, email: string) {
    let existing: MonitorCred | null = null;
    if (email) {
      try {
        existing = await api.getMonitorCred(email);
      } catch {
        existing = null;
      }
    }
    modal = {
      kind: "creds",
      profileName,
      email: existing?.email ?? email,
      password: existing?.password ?? "",
      orgId: existing?.orgId ?? "",
      allocs: null,
      detecting: false,
      busy: false,
    };
  }
  function openCredsForProfile(p: ProfileInfo) {
    openCreds(p.name, p.email);
  }
  function openCredsForCurrent() {
    openCreds(null, currentEmail ?? "");
  }

  async function detectOrg() {
    if (modal?.kind !== "creds") return;
    const email = modal.email.trim();
    if (!email || !modal.password) {
      toast("请先填写邮箱和密码", "err");
      return;
    }
    modal.detecting = true;
    try {
      const list = await api.listAllocations(email, modal.password);
      if (modal?.kind !== "creds") return;
      if (list.length === 0) {
        toast("该账号下没有拼车套餐", "warn");
        modal.allocs = null;
      } else if (list.length === 1) {
        modal.orgId = list[0].id;
        modal.allocs = null;
        toast(`已探测到组织 ID ${list[0].id}`);
      } else {
        modal.allocs = list;
      }
    } catch (e) {
      toast(String(e), "err");
    } finally {
      if (modal?.kind === "creds") modal.detecting = false;
    }
  }
  function pickAlloc(a: Allocation) {
    if (modal?.kind !== "creds") return;
    modal.orgId = a.id;
    modal.allocs = null;
  }

  async function confirmCreds() {
    if (modal?.kind !== "creds") return;
    const email = modal.email.trim();
    if (!email || !modal.password) {
      toast("请填写邮箱和密码", "err");
      return;
    }
    modal.busy = true;
    const c: MonitorCred = { email, password: modal.password, orgId: modal.orgId.trim() };
    try {
      await api.setMonitorCred(c, modal.profileName);
      toast("监控凭证已保存");
      modal = null;
      await refreshAll();
    } catch (e) {
      toast(String(e), "err");
      if (modal?.kind === "creds") modal.busy = false;
    }
  }

  // ---- lifecycle ----
  // tickTimer 仅在窗口可见时跑（每秒只为更新倒计时显示，隐藏时无人看，省电）
  let tickTimer: ReturnType<typeof setInterval> | undefined;
  let followTimer: ReturnType<typeof setInterval>;
  let unlistenClose: (() => void) | undefined;
  // Svelte 5 注:onCloseRequested 的 closure 内 `closeAction === "background"` 是动态读取，
  // 因为 $state 编译为 proxy getter——用户切了设置立刻生效，无需重绑 listener。

  function startTick() {
    if (tickTimer) return;
    now = Date.now();
    tickTimer = setInterval(() => (now = Date.now()), 1000);
  }
  function stopTick() {
    if (tickTimer) {
      clearInterval(tickTimer);
      tickTimer = undefined;
    }
  }
  function onVisChange() {
    if (document.hidden) stopTick();
    else startTick();
  }

  // 最小化：按模式 → 悬浮球 或 菜单栏圆环（托盘常驻，仅最小化主窗口）
  async function enterFloat() {
    try {
      if (floatMode === "tray") {
        // 圆环模式：Rust 后台循环自己更新图标，这里只隐藏主窗口
        await api.minimizeMain();
      } else {
        await api.minimizeToFloat(floatSize);
      }
    } catch (e) {
      toast(String(e), "err");
    }
  }

  // ---- 菜单栏圆环托盘（Rust 端自取数据 + tiny-skia 自绘）----
  // 前端只切开关 + 传刷新间隔，圆环渲染/刷新全在 Rust 后台循环里完成（主窗口可全程隐藏）
  async function applyTrayMode() {
    try {
      if (floatMode === "tray") {
        await api.hideFloat(); // 与悬浮球互斥
        await api.setTrayMode(true, refreshSec);
      } else {
        await api.setTrayMode(false, refreshSec);
      }
    } catch {
      /* ignore */
    }
  }

  // 头部刷新：按当前页刷新——监控页刷新额度/服务，用量页重载用量
  function headerRefresh() {
    if (view === "usage") usageReloadKey++;
    else refreshAll();
  }

  function openSettings() {
    modal = {
      kind: "settings",
      sec: refreshSec,
      mode: floatMode,
      size: floatSize,
      closeAct: closeAction,
    };
  }
  function setSec(s: number) {
    if (modal?.kind === "settings") modal.sec = s;
  }
  function setMode(m: "ball" | "tray") {
    if (modal?.kind === "settings") modal.mode = m;
  }
  function setCloseAct(a: "quit" | "background") {
    if (modal?.kind === "settings") modal.closeAct = a;
  }
  function confirmSettings() {
    if (modal?.kind !== "settings") return;
    const sec = Math.round(modal.sec);
    const size = Math.round(modal.size);
    if (!Number.isFinite(sec) || sec < 5 || sec > 3600) {
      toast("刷新间隔需在 5–3600 秒之间", "err");
      return;
    }
    if (!Number.isFinite(size) || size < 30 || size > 600) {
      toast("悬浮球大小需在 30–600 之间", "err");
      return;
    }
    refreshSec = sec;
    floatMode = modal.mode;
    floatSize = size;
    closeAction = modal.closeAct;
    try {
      localStorage.setItem("refreshSec", String(sec));
      localStorage.setItem("floatMode", floatMode);
      localStorage.setItem("floatSize", String(size));
      localStorage.setItem("closeAction", closeAction);
    } catch {
      /* ignore */
    }
    // 若悬浮球当前可见，立即按新尺寸调整
    api.resizeFloat(size).catch(() => {});
    // 持久化供下次启动时 Rust 读取（并把 refreshSec 同步到 ui.json 供悬浮球读）
    api.saveUiConfig(floatMode, floatSize, refreshSec).catch(() => {});
    // 应用/切换菜单栏圆环显示
    applyTrayMode();
    modal = null;
    toast("设置已保存");
  }

  onMount(async () => {
    if (isFloat) return;
    applyTrayMode();
    // 圆环模式：本次 onMount 是 Rust 为渲染圆环而显示的主窗口 → 渲染后隐藏主窗口。
    // 悬浮球模式：球已由 Rust 显示，主窗口全程隐藏，onMount 只在用户点开主窗口时运行，不隐藏。
    if (floatMode === "tray") {
      await api.hideMain();
    }

    try {
      const stored = localStorage.getItem("noApp");
      if (stored !== null) noApp = stored === "1";
    } catch {
      /* ignore */
    }
    try {
      env = await api.getEnv();
    } catch (e) {
      console.error(e);
    }
    await refreshAll();
    startTick();
    document.addEventListener("visibilitychange", onVisChange);
    followTimer = setInterval(follow, FOLLOW_MS);

    // 关闭按钮：退出程序 或 后台运行
    try {
      unlistenClose = await getCurrentWindow().onCloseRequested((event) => {
        if (closeAction === "background") {
          event.preventDefault();
          enterFloat();
        } else {
          api.quitApp();
        }
      });
    } catch (e) {
      console.error(e);
    }
  });

  onDestroy(() => {
    stopTick();
    clearInterval(followTimer);
    document.removeEventListener("visibilitychange", onVisChange);
    unlistenClose?.();
  });

  // 刷新定时器：随 refreshSec 变化重建
  $effect(() => {
    if (isFloat) return;
    const ms = Math.max(5, refreshSec) * 1000;
    const t = setInterval(() => {
      doRefresh();
      fetchAccountQuotas();
    }, ms);
    return () => clearInterval(t);
  });


  $effect(() => {
    if (isFloat) return;
    try {
      localStorage.setItem("noApp", noApp ? "1" : "0");
    } catch {
      /* ignore */
    }
  });

  // 持久化当前标签页（监控/用量）
  $effect(() => {
    if (isFloat) return;
    try {
      localStorage.setItem("view", view);
    } catch {
      /* ignore */
    }
  });
</script>

{#if isFloat}
  <FloatWidget />
{:else}
<div class="app">
  <header>
    <div class="brand">
      <div class="logo"><Logo size={38} /></div>
      <div class="t">Reclaude 控制台</div>
    </div>
    <div class="head-actions">
      <button class="iconbtn" onclick={openSettings} title="设置刷新间隔" aria-label="设置">
        <Settings size={17} />
      </button>
      <button
        class="iconbtn"
        onclick={enterFloat}
        title={floatMode === "tray" ? "最小化到菜单栏圆环" : "最小化为悬浮球"}
        aria-label="最小化"
      >
        <PictureInPicture2 size={17} />
      </button>
      <button class="refresh" class:spin={refreshing && view !== "usage"} onclick={headerRefresh} title="刷新" aria-label="刷新">
        <RefreshCw size={17} />
      </button>
    </div>
  </header>

  {#if env && !env.reclaudeFound}
    <div class="banner">
      <AlertTriangle size={15} />
      <span>未找到 reclaude，账号切换不可用（监控仍可用）。</span>
    </div>
  {/if}

  <nav class="tabs">
    <button class:on={view === "monitor"} onclick={() => (view = "monitor")}>监控</button>
    <button class:on={view === "usage"} onclick={() => (view = "usage")}>用量</button>
  </nav>

  {#if view === "usage"}
    <UsageView {cred} reloadKey={usageReloadKey} onConfigure={openCredsForCurrent} />
  {:else}
  <!-- ======== HERO：当前账号 ======== -->
  <section class="hero">
    {#if !currentEmail}
      <div class="hero-empty">
        <LogIn size={26} />
        <div class="he-title">未检测到当前账号</div>
        <div class="he-sub">请先用 reclaude 登录，或从下方切换到某个档案。</div>
      </div>
    {:else}
      <div class="hero-head">
        <div class="who">
          <div class="who-name">{currentName ?? currentEmail}</div>
          {#if currentName}<div class="who-mail">{currentEmail}</div>{/if}
        </div>
        {#if heroBadge}
          <span class="badge {heroBadge.level}"><span class="bdot"></span>{heroBadge.text}</span>
        {/if}
      </div>

      {#if snapshot?.quota}
        {@const q = snapshot.quota}
        <div class="gauge-wrap">
          <Gauge
            ratio={q.ratio}
            big={`${q.pct.toFixed(0)}%`}
            small="已用"
            color={quotaColor(q.ratio)}
          />
        </div>
        <div class="amount">{fmtUsd(q.usedUsd)} <span class="of">/ {fmtUsd(q.totalUsd)}</span></div>
        <div class="meta">
          <div class="m">
            <Wallet size={14} />
            <span>剩余 <b style="color:var(--ok)">{fmtUsd(q.remainingUsd)}</b></span>
          </div>
          {#if resetText}
            <div class="m">
              <Clock size={14} />
              <span><b>{resetText}</b> 后重置</span>
            </div>
          {/if}
        </div>
        {#if snapshot.metrics}
          {@const m = snapshot.metrics}
          <div class="service">
            <span class="sdot {m.stateLevel}"></span>
            <span class="stext">服务{m.stateText}</span>
            <span class="sep">·</span>
            <span>错误率 <b>{m.errorRatePct.toFixed(2)}%</b></span>
            <span class="sep">·</span>
            <span>延迟 <b>{fmtMs(m.avgLatencyMs)}</b></span>
          </div>
        {/if}
      {:else}
        <div class="hero-config">
          <div class="hc-msg">{heroMessage}</div>
          {#if !cred}
            <button class="cta" onclick={openCredsForCurrent}>
              <KeyRound size={15} /> 配置监控凭证
            </button>
          {/if}
        </div>
      {/if}
    {/if}
  </section>

  <!-- ======== 切换账号 ======== -->
  <section class="switch">
    <div class="sec-head">
      <span class="sec-title">切换账号</span>
      <span class="count">{profiles.length}</span>
    </div>

    {#if profiles.length === 0}
      <div class="list-empty">
        还没有保存任何档案。<br />
        把当前登录的账号保存为档案，之后就能一键切回。
      </div>
    {:else}
      <div class="list">
        {#each profiles as p (p.name)}
          <AccountRow
            name={p.name}
            email={p.email}
            isActive={isActiveEmail(p.email)}
            hasCreds={p.hasMonitor}
            quota={quotaFor(p)}
            loading={loadingFor(p)}
            errorText={errTextFor(p)}
            busy={busyName === p.name}
            onUse={() => openUse(p)}
            onConfig={() => openCredsForProfile(p)}
            onRemove={() => openRemove(p)}
          />
        {/each}
      </div>
    {/if}

    <button class="save-btn" disabled={!currentEmail} onclick={openSave}>
      <Plus size={17} /> 保存当前账号为档案
    </button>
    <p class="save-hint">把当前登录账号的身份与桌面会话整套快照下来，方便随时切回。</p>
  </section>

  <footer>
    <label class="toggle">
      <input type="checkbox" bind:checked={noApp} />
      <span>切换时只换凭证，不自动打开桌面 App</span>
    </label>
    {#if lastUpdated}
      <span class="ts">更新于 {new Date(lastUpdated).toLocaleTimeString("zh-CN")}</span>
    {/if}
  </footer>
  {/if}
</div>

<!-- toasts -->
<div class="toasts">
  {#each toasts as t (t.id)}
    <div class="toast {t.level}">{t.text}</div>
  {/each}
</div>

<!-- modals -->
{#if modal?.kind === "save"}
  <Modal title="保存当前账号为档案" onClose={() => (modal = null)}>
    <div class="field">
      <label for="m-name">档案名</label>
      <input id="m-name" type="text" placeholder="例如 work / home" bind:value={modal.name} />
    </div>
    <label class="cbox">
      <input type="checkbox" bind:checked={modal.incMon} />
      <span>同时保存监控凭证（用于额度监控）</span>
    </label>
    {#if modal.incMon}
      <div class="field"><label for="m-email">邮箱</label><input id="m-email" type="text" bind:value={modal.email} /></div>
      <div class="field"><label for="m-pass">密码</label><input id="m-pass" type="password" bind:value={modal.password} /></div>
      <div class="field"><label for="m-org">组织 ID（可留空，自动探测）</label><input id="m-org" type="text" placeholder="例如 2440" bind:value={modal.orgId} /></div>
    {/if}
    <div class="modal-foot">
      <button class="primary" disabled={modal.busy} onclick={confirmSave}>{modal.busy ? "保存中…" : "保存快照"}</button>
      <button class="cancel" onclick={() => (modal = null)}>取消</button>
    </div>
  </Modal>
{:else if modal?.kind === "creds"}
  <Modal title="监控凭证" onClose={() => (modal = null)}>
    <div class="field"><label for="c-email">邮箱</label><input id="c-email" type="text" bind:value={modal.email} /></div>
    <div class="field"><label for="c-pass">密码</label><input id="c-pass" type="password" bind:value={modal.password} /></div>
    <div class="field">
      <label for="c-org">组织 ID</label>
      <div class="org-row">
        <input id="c-org" type="text" placeholder="留空将自动探测" bind:value={modal.orgId} />
        <button class="detect" disabled={modal.detecting} onclick={detectOrg}>{modal.detecting ? "探测中…" : "自动探测"}</button>
      </div>
    </div>
    {#if modal.allocs}
      <div class="alloc-list">
        <div class="alloc-hint">该账号有多个拼车套餐，选择一个：</div>
        {#each modal.allocs as a (a.id)}
          <button class="alloc" onclick={() => pickAlloc(a)}>
            <span>{a.label}</span>{#if a.capacity}<span class="cap">{a.capacity} 人</span>{/if}
          </button>
        {/each}
      </div>
    {/if}
    <div class="modal-foot">
      <button class="primary" disabled={modal.busy} onclick={confirmCreds}>{modal.busy ? "保存中…" : "保存"}</button>
      <button class="cancel" onclick={() => (modal = null)}>取消</button>
    </div>
    <p class="hint">{#if modal.profileName}保存到档案 “{modal.profileName}” 的 monitor.json{:else}保存为当前账号的监控凭证{/if}</p>
  </Modal>
{:else if modal?.kind === "use"}
  <Modal title="切换账号" onClose={() => (modal = null)}>
    <p class="confirm">将切换到 <b>{modal.profile.name}</b><br /><span class="cmuted">{modal.profile.email}</span></p>
    <p class="hint">会停止桌面 App 与 daemon，写入该账号凭证{modal.profile.hasAppSession ? "、恢复 App 会话" : ""}，然后{noApp ? "仅拉起 daemon" : "打开桌面 App"}。</p>
    <label class="cbox"><input type="checkbox" bind:checked={noApp} /><span>只换凭证，不自动打开桌面 App</span></label>
    <div class="modal-foot">
      <button class="primary" onclick={confirmUse}>确认切换</button>
      <button class="cancel" onclick={() => (modal = null)}>取消</button>
    </div>
  </Modal>
{:else if modal?.kind === "remove"}
  <Modal title="删除档案" onClose={() => (modal = null)}>
    <p class="confirm">确定删除档案 <b>{modal.profile.name}</b>？<br /><span class="cmuted">{modal.profile.email}</span></p>
    <p class="hint">仅删除本地快照与其监控凭证，不影响当前已登录的账号。</p>
    <div class="modal-foot">
      <button class="danger" disabled={modal.busy} onclick={confirmRemove}>{modal.busy ? "删除中…" : "删除"}</button>
      <button class="cancel" onclick={() => (modal = null)}>取消</button>
    </div>
  </Modal>
{:else if modal?.kind === "settings"}
  <Modal title="设置" onClose={() => (modal = null)}>
    <div class="field">
      <label for="s-sec">自动刷新间隔（秒）</label>
      <input id="s-sec" type="number" min="5" max="3600" step="5" bind:value={modal.sec} />
    </div>
    <div class="presets">
      {#each [10, 30, 60, 300] as s (s)}
        <button class="preset" class:on={modal.sec === s} onclick={() => setSec(s)}>
          {s < 60 ? `${s}s` : `${s / 60}m`}
        </button>
      {/each}
    </div>
    <p class="hint">额度与服务指标的自动刷新频率（5–3600 秒）。倒计时与跟随账号不受影响。</p>

    <div class="field set-sep">
      <div class="seg-title">最小化方式</div>
      <div class="presets">
        <button class="preset" class:on={modal.mode === "ball"} onclick={() => setMode("ball")}>悬浮球</button>
        <button class="preset" class:on={modal.mode === "tray"} onclick={() => setMode("tray")}>菜单栏圆环</button>
      </div>
    </div>
    {#if modal.mode === "ball"}
      <div class="field">
        <label for="s-size">悬浮球大小（px，正方形）</label>
        <input id="s-size" type="number" min="30" max="600" step="10" bind:value={modal.size} />
      </div>
    {:else}
      <p class="hint">菜单栏圆环显示「可用余额百分比」，点击图标打开主面板。</p>
    {/if}

    <div class="field set-sep">
      <div class="seg-title">关闭窗口时</div>
      <div class="presets">
        <button class="preset" class:on={modal.closeAct === "quit"} onclick={() => setCloseAct("quit")}>退出程序</button>
        <button class="preset" class:on={modal.closeAct === "background"} onclick={() => setCloseAct("background")}>后台运行</button>
      </div>
    </div>

    <div class="modal-foot">
      <button class="primary" onclick={confirmSettings}>保存</button>
      <button class="cancel" onclick={() => (modal = null)}>取消</button>
    </div>
  </Modal>
{/if}
{/if}

<style>
  .app {
    max-width: 480px;
    margin: 0 auto;
    padding: 16px 18px 26px;
    height: 100vh;
    overflow-y: auto;
    box-sizing: border-box;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 16px;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 11px;
  }
  .logo {
    width: 38px;
    height: 38px;
    display: grid;
    place-items: center;
  }
  .brand .t {
    font-size: 16.5px;
    font-weight: 800;
    letter-spacing: -0.01em;
  }
  .head-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .refresh,
  .iconbtn {
    width: 38px;
    height: 38px;
    border-radius: 11px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--muted);
    cursor: pointer;
    display: grid;
    place-items: center;
    transition: all 0.15s ease;
  }
  .refresh:hover,
  .iconbtn:hover {
    color: var(--accent);
    border-color: var(--accent);
  }
  .refresh.spin :global(svg) {
    animation: spin 0.9s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .banner {
    display: flex;
    align-items: center;
    gap: 8px;
    background: var(--warn-soft);
    color: var(--warn);
    border-radius: 11px;
    padding: 9px 13px;
    font-size: 12.5px;
    margin-bottom: 14px;
  }

  .tabs {
    display: flex;
    gap: 4px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 3px;
    margin-bottom: 16px;
  }
  .tabs button {
    flex: 1;
    border: none;
    background: transparent;
    color: var(--muted);
    font-size: 13.5px;
    font-weight: 600;
    padding: 8px;
    border-radius: 9px;
    cursor: pointer;
    transition: all 0.15s ease;
  }
  .tabs button.on {
    background: var(--accent);
    color: #fff;
  }

  /* hero */
  .hero {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 20px;
    padding: 20px;
    text-align: center;
    box-shadow: var(--shadow);
    margin-bottom: 18px;
  }
  .hero-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    text-align: left;
    margin-bottom: 6px;
  }
  .who {
    min-width: 0;
  }
  .who-name {
    font-size: 16px;
    font-weight: 800;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .who-mail {
    font-size: 12px;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 11px;
    border-radius: 999px;
    font-size: 12px;
    font-weight: 700;
    flex-shrink: 0;
  }
  .badge .bdot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: currentColor;
  }
  .badge.ok {
    background: var(--ok-soft);
    color: var(--ok);
  }
  .badge.warn {
    background: var(--warn-soft);
    color: var(--warn);
  }
  .badge.err {
    background: var(--err-soft);
    color: var(--err);
  }
  .gauge-wrap {
    margin: 8px 0 4px;
  }
  .amount {
    font-size: 24px;
    font-weight: 800;
    letter-spacing: -0.02em;
    margin-top: 2px;
  }
  .amount .of {
    color: var(--faint);
    font-weight: 600;
    font-size: 18px;
  }
  .meta {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 18px;
    margin-top: 10px;
    flex-wrap: wrap;
  }
  .meta .m {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    color: var(--muted);
  }
  .meta .m :global(svg) {
    color: var(--faint);
  }
  .service {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    flex-wrap: wrap;
    justify-content: center;
    margin-top: 14px;
    padding-top: 13px;
    border-top: 1px solid var(--border);
    font-size: 12.5px;
    color: var(--muted);
    width: 100%;
  }
  .service b {
    color: var(--fg);
    font-weight: 700;
  }
  .service .sep {
    color: var(--faint);
  }
  .sdot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }
  .sdot.ok {
    background: var(--ok);
  }
  .sdot.warn {
    background: var(--warn);
  }
  .sdot.err {
    background: var(--err);
  }
  .stext {
    color: var(--fg);
    font-weight: 600;
  }
  .hero-empty {
    padding: 18px 8px;
    color: var(--muted);
  }
  .hero-empty :global(svg) {
    color: var(--faint);
  }
  .he-title {
    font-size: 15px;
    font-weight: 700;
    color: var(--fg);
    margin: 8px 0 4px;
  }
  .he-sub {
    font-size: 12.5px;
  }
  .hero-config {
    padding: 22px 8px 10px;
  }
  .hc-msg {
    color: var(--muted);
    font-size: 13px;
    margin-bottom: 14px;
  }
  .cta {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    border: none;
    background: var(--accent);
    color: #fff;
    padding: 9px 16px;
    border-radius: 11px;
    font-weight: 600;
    font-size: 13px;
    cursor: pointer;
  }
  .cta:hover {
    filter: brightness(1.08);
  }

  /* switch section */
  .sec-head {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 4px 11px;
  }
  .sec-title {
    font-size: 14px;
    font-weight: 700;
    color: var(--muted);
  }
  .count {
    font-size: 11.5px;
    font-weight: 700;
    color: var(--faint);
    background: var(--surface-2);
    border-radius: 999px;
    padding: 1px 9px;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 9px;
  }
  .list-empty {
    color: var(--muted);
    font-size: 12.5px;
    line-height: 1.8;
    text-align: center;
    padding: 14px;
    border: 1px dashed var(--border-strong);
    border-radius: 15px;
  }
  .save-btn {
    width: 100%;
    margin-top: 12px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 12px;
    border-radius: 14px;
    border: 1px dashed var(--border-strong);
    background: transparent;
    color: var(--fg);
    font-size: 13.5px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.15s ease;
  }
  .save-btn:hover:not(:disabled) {
    border-color: var(--accent);
    color: var(--accent);
    background: var(--accent-soft);
  }
  .save-btn:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .save-hint {
    font-size: 11.5px;
    color: var(--faint);
    text-align: center;
    margin: 9px 6px 0;
    line-height: 1.6;
  }
  footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-top: 20px;
    padding-top: 14px;
    border-top: 1px solid var(--border);
  }
  .toggle {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--muted);
    cursor: pointer;
  }
  .toggle input {
    width: 15px;
    height: 15px;
    accent-color: var(--accent);
  }
  .ts {
    font-size: 11px;
    color: var(--faint);
    white-space: nowrap;
  }

  /* modal internals */
  .field {
    margin-bottom: 11px;
  }
  .field label {
    display: block;
    font-size: 12px;
    color: var(--muted);
    margin-bottom: 5px;
  }
  .field input {
    width: 100%;
    padding: 9px 11px;
    font-size: 13px;
    border-radius: 9px;
    background: var(--surface-2);
    color: var(--fg);
    border: 1px solid var(--border-strong);
    outline: none;
  }
  .field input:focus {
    border-color: var(--accent);
  }
  .presets {
    display: flex;
    gap: 7px;
    margin: 10px 0 2px;
  }
  .preset {
    flex: 1;
    padding: 7px 0;
    border-radius: 9px;
    border: 1px solid var(--border-strong);
    background: var(--surface-2);
    color: var(--muted);
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
    transition:
      border-color 0.15s ease,
      color 0.15s ease,
      background 0.15s ease;
  }
  .preset:hover {
    border-color: var(--accent);
    color: var(--fg);
  }
  .preset.on {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }
  .set-sep {
    margin-top: 14px;
    padding-top: 14px;
    border-top: 1px solid var(--border);
  }
  .seg-title {
    font-size: 12px;
    color: var(--muted);
    margin-bottom: 7px;
  }
  .cbox {
    display: flex;
    align-items: center;
    gap: 9px;
    font-size: 12.5px;
    margin: 4px 0 12px;
    cursor: pointer;
  }
  .cbox input {
    width: 15px;
    height: 15px;
    accent-color: var(--accent);
  }
  .org-row {
    display: flex;
    gap: 7px;
  }
  .org-row input {
    flex: 1;
  }
  .detect {
    white-space: nowrap;
    border: 1px solid var(--border-strong);
    background: var(--surface-2);
    color: var(--fg);
    border-radius: 9px;
    padding: 0 13px;
    cursor: pointer;
    font-size: 12.5px;
  }
  .detect:hover:not(:disabled) {
    border-color: var(--accent);
    color: var(--accent);
  }
  .detect:disabled {
    opacity: 0.6;
  }
  .alloc-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-bottom: 11px;
  }
  .alloc-hint {
    font-size: 12px;
    color: var(--muted);
  }
  .alloc {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    padding: 9px 12px;
    border: 1px solid var(--border-strong);
    border-radius: 9px;
    background: var(--surface-2);
    color: var(--fg);
    cursor: pointer;
    font-size: 12.5px;
    text-align: left;
  }
  .alloc:hover {
    border-color: var(--accent);
  }
  .cap {
    color: var(--muted);
  }
  .modal-foot {
    display: flex;
    gap: 8px;
    margin-top: 6px;
  }
  .modal-foot button {
    padding: 9px 16px;
    border-radius: 9px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    border: 1px solid transparent;
  }
  .primary {
    background: var(--accent);
    color: #fff;
  }
  .primary:hover:not(:disabled) {
    filter: brightness(1.08);
  }
  .primary:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .danger {
    background: var(--err);
    color: #fff;
  }
  .danger:hover:not(:disabled) {
    filter: brightness(1.05);
  }
  .cancel {
    background: var(--surface-2);
    color: var(--fg);
    border: 1px solid var(--border-strong);
  }
  .cancel:hover {
    border-color: var(--muted);
  }
  .hint {
    font-size: 11.5px;
    color: var(--faint);
    margin: 12px 0 0;
    line-height: 1.6;
  }
  .confirm {
    font-size: 14px;
    margin: 0 0 4px;
    line-height: 1.6;
  }
  .cmuted {
    color: var(--muted);
    font-size: 12.5px;
  }

  .toasts {
    position: fixed;
    bottom: 18px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: 60;
    align-items: center;
  }
  .toast {
    padding: 9px 16px;
    border-radius: 11px;
    font-size: 13px;
    font-weight: 600;
    box-shadow: var(--shadow);
    background: var(--surface);
    border: 1px solid var(--border-strong);
    color: var(--fg);
    animation: rise 0.16s ease;
    max-width: 80vw;
  }
  .toast.ok {
    border-color: var(--ok);
    color: var(--ok);
  }
  .toast.err {
    border-color: var(--err);
    color: var(--err);
  }
  .toast.warn {
    border-color: var(--warn);
    color: var(--warn);
  }
  @keyframes rise {
    from {
      transform: translateY(6px);
      opacity: 0;
    }
    to {
      transform: none;
      opacity: 1;
    }
  }
</style>
