<script lang="ts">
  // 托盘面板：菜单栏 popover 风格的紧凑信息卡。独立窗口/上下文，自取数据（仿 FloatWidget），
  // 不与主窗口共享 store。强调信息密度：细进度条 + 额度/服务多指标紧排，而非大圆环。
  import { onMount, onDestroy } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { PanelTopOpen, Settings } from "@lucide/svelte";
  import { api, type MonitorSnapshot } from "$lib/api";
  import { fetchFloatSnapshot, getFloatIntervalMs } from "$lib/float";
  import {
    fmtUsdCompact,
    fmtMs,
    fmtCountdown,
    fmtInt,
    fmtTokens,
    quotaColor,
    QUOTA_ERR_RATIO,
    QUOTA_WARN_RATIO,
  } from "$lib/format";

  let snapshot = $state<MonitorSnapshot | null>(null);
  let email = $state<string | null>(null);
  let status = $state<"loading" | "ok" | "nocred" | "error">("loading");
  let now = $state(Date.now());

  let timer: ReturnType<typeof setInterval> | undefined;
  let tick: ReturnType<typeof setInterval> | undefined;
  let unlistenFocus: (() => void) | undefined;
  let intervalMs = 30_000;

  async function load() {
    try {
      email = await api.currentAccount();
      const res = await fetchFloatSnapshot();
      if (res.kind === "nocred") {
        status = "nocred";
        return;
      }
      const snap = res.snapshot;
      snapshot = snap;
      if (snap.badCredentials) {
        status = "error";
        return;
      }
      status = snap.quota || snap.metrics ? "ok" : "error";
    } catch {
      status = snapshot ? "ok" : "error";
    }
  }

  const quota = $derived(snapshot?.quota ?? null);
  const metrics = $derived(snapshot?.metrics ?? null);
  const usedPct = $derived(quota ? Math.max(0, Math.min(100, quota.ratio * 100)) : 0);
  const barColor = $derived(quota ? quotaColor(quota.ratio) : "var(--accent)");
  const resetText = $derived(quota?.resetAtMs ? fmtCountdown(quota.resetAtMs - now) : "");

  const level = $derived.by(() => {
    if (snapshot?.badCredentials) return "err";
    if (quota && quota.ratio >= QUOTA_ERR_RATIO) return "err";
    if (metrics?.stateLevel === "err") return "err";
    if (metrics?.stateLevel === "warn") return "warn";
    if (quota && quota.ratio >= QUOTA_WARN_RATIO) return "warn";
    return "ok";
  });

  function start() {
    now = Date.now();
    void load();
    timer ??= setInterval(load, intervalMs);
    tick ??= setInterval(() => (now = Date.now()), 1000);
  }
  function stop() {
    if (timer) {
      clearInterval(timer);
      timer = undefined;
    }
    if (tick) {
      clearInterval(tick);
      tick = undefined;
    }
  }

  onMount(async () => {
    // 透明窗口：html/body 透明且不滚动（用 JS 设，避免污染主窗口全局样式）
    for (const el of [document.documentElement, document.body]) {
      el.style.setProperty("background", "transparent", "important");
      el.style.setProperty("overflow", "hidden", "important");
    }
    intervalMs = await getFloatIntervalMs();
    start();
    // 显示=获得焦点时刷新；失焦时停轮询（失焦后 Rust 会隐藏面板）
    try {
      unlistenFocus = await getCurrentWindow().onFocusChanged(({ payload: focused }) => {
        if (focused) start();
        else stop();
      });
    } catch {
      /* ignore */
    }
  });
  onDestroy(() => {
    stop();
    unlistenFocus?.();
  });
</script>

<div class="wrap">
  <div class="card">
    <div class="hd">
      <span class="dot {level}" class:hide={status === "loading"}></span>
      <span class="name" title={email ?? ""}>{email ?? "未登录"}</span>
      <button class="ic" onclick={() => api.openMain(false)} title="打开主面板" aria-label="打开主面板">
        <PanelTopOpen size={15} />
      </button>
      <button class="ic" onclick={() => api.openMain(true)} title="设置" aria-label="设置">
        <Settings size={15} />
      </button>
    </div>

    {#if status === "ok" && quota}
      <div class="amt">
        <span class="big">{fmtUsdCompact(quota.usedUsd)}</span>
        <span class="of">/ {fmtUsdCompact(quota.totalUsd)}</span>
        <span class="remain">剩余 <b>{fmtUsdCompact(quota.remainingUsd)}</b></span>
      </div>
      <div class="bar"><div class="fill" style="width:{usedPct}%;background:{barColor}"></div></div>
      <div class="sub">
        <span>已用 <b>{quota.pct.toFixed(0)}%</b></span>
        {#if resetText}<span>{resetText} 后重置</span>{/if}
      </div>

      <div class="grid">
        {#if metrics}
          <div class="cell"><span class="l">错误率</span><span class="v {metrics.stateLevel}">{metrics.errorRatePct.toFixed(2)}%</span></div>
          <div class="cell"><span class="l">延迟</span><span class="v">{fmtMs(metrics.avgLatencyMs)}</span></div>
          <div class="cell"><span class="l">RPM</span><span class="v">{fmtInt(metrics.rpm)}</span></div>
          <div class="cell"><span class="l">TPM</span><span class="v">{fmtTokens(metrics.tpm)}</span></div>
        {:else}
          <div class="cell wide"><span class="l">服务指标</span><span class="v">暂不可用</span></div>
        {/if}
      </div>
    {:else}
      <div class="state">
        <span class="stext">
          {status === "loading"
            ? "加载中…"
            : status === "nocred"
              ? "未配置监控凭证"
              : snapshot?.badCredentials
                ? "账号或密码错误"
                : status === "ok"
                  ? "该账号无拼车额度"
                  : "额度获取失败"}
        </span>
        <button class="open" onclick={() => api.openMain(status === "nocred")}>
          {status === "nocred" ? "去配置" : "打开主面板"}
        </button>
      </div>
    {/if}
  </div>
</div>

<style>
  /* 透明窗口留出 gutter 让卡片投影可见（卡片小于窗口） */
  .wrap {
    width: 100vw;
    height: 100vh;
    padding: 10px 12px 14px;
    box-sizing: border-box;
    background: transparent;
  }
  .card {
    width: 100%;
    height: 100%;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 13px;
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: 14px;
    box-shadow: 0 10px 26px rgba(0, 0, 0, 0.4);
    overflow: hidden;
  }

  .hd {
    display: flex;
    align-items: center;
    gap: 7px;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
    background: var(--ok);
  }
  .dot.warn {
    background: var(--warn);
  }
  .dot.err {
    background: var(--err);
  }
  .dot.hide {
    opacity: 0;
  }
  .name {
    flex: 1;
    min-width: 0;
    font-size: 12.5px;
    font-weight: 700;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ic {
    flex-shrink: 0;
    width: 24px;
    height: 24px;
    padding: 0; /* 重置 button UA 默认内边距，否则不对称内边距会把图标挤偏 */
    display: grid;
    place-items: center;
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--muted);
    border-radius: 7px;
    cursor: pointer;
    transition:
      color 0.15s ease,
      border-color 0.15s ease;
  }
  .ic :global(svg) {
    display: block; /* 去掉 inline svg 的基线间隙，保证在按钮内精确居中 */
  }
  .ic:hover {
    color: var(--accent);
    border-color: var(--accent);
  }

  .amt {
    display: flex;
    align-items: baseline;
    gap: 5px;
  }
  .amt .big {
    font-size: 19px;
    font-weight: 800;
    letter-spacing: -0.02em;
  }
  .amt .of {
    font-size: 12.5px;
    color: var(--faint);
    font-weight: 600;
  }
  .amt .remain {
    margin-left: auto;
    font-size: 12px;
    color: var(--muted);
  }
  .amt .remain b {
    color: var(--ok);
    font-weight: 700;
  }

  .bar {
    height: 6px;
    border-radius: 999px;
    background: var(--track);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    border-radius: 999px;
    transition:
      width 0.5s cubic-bezier(0.4, 0, 0.2, 1),
      background 0.3s ease;
  }
  .sub {
    display: flex;
    justify-content: space-between;
    font-size: 11.5px;
    color: var(--muted);
  }
  .sub b {
    color: var(--fg);
    font-weight: 700;
  }

  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr 1fr 1fr;
    gap: 6px;
    margin-top: 2px;
    padding-top: 9px;
    border-top: 1px solid var(--border);
  }
  .cell {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .cell.wide {
    grid-column: 1 / -1;
    flex-direction: row;
    justify-content: space-between;
    align-items: center;
  }
  .cell .l {
    font-size: 10.5px;
    color: var(--faint);
  }
  .cell .v {
    font-size: 13px;
    font-weight: 700;
    color: var(--fg);
    font-variant-numeric: tabular-nums;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cell .v.warn {
    color: var(--warn);
  }
  .cell .v.err {
    color: var(--err);
  }

  .state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 11px;
    color: var(--muted);
  }
  .stext {
    font-size: 13px;
  }
  .open {
    border: none;
    background: var(--accent);
    color: #fff;
    padding: 7px 16px;
    border-radius: 9px;
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
  }
  .open:hover {
    filter: brightness(1.08);
  }
</style>
