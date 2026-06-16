<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { Clock, KeyRound, LogIn, Wallet } from "@lucide/svelte";
  import {
    fmtCountdown,
    fmtMs,
    fmtUsd,
    quotaColor,
    QUOTA_ERR_RATIO,
    QUOTA_WARN_RATIO,
  } from "$lib/format";
  import { monitor, currentProfileName, openCredsForCurrent } from "$lib/monitor.svelte";
  import Gauge from "./Gauge.svelte";

  // 倒计时秒针：仅在窗口可见时跑（隐藏时无人看，省电）
  let now = $state(Date.now());
  let tickTimer: ReturnType<typeof setInterval> | undefined;

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

  onMount(() => {
    startTick();
    document.addEventListener("visibilitychange", onVisChange);
  });
  onDestroy(() => {
    stopTick();
    document.removeEventListener("visibilitychange", onVisChange);
  });

  const currentName = $derived(currentProfileName());

  const heroBadge = $derived.by(() => {
    if (!monitor.cred) return null;
    const q = monitor.snapshot?.quota;
    const m = monitor.snapshot?.metrics;
    if (monitor.snapshot?.badCredentials) return { level: "err", text: "密码错误" };
    if (q && q.ratio >= QUOTA_ERR_RATIO) return { level: "err", text: "额度告急" };
    if (m && m.stateLevel === "err") return { level: "err", text: "服务故障" };
    if (m && m.stateLevel === "warn") return { level: "warn", text: "服务抖动" };
    if (q && q.ratio >= QUOTA_WARN_RATIO) return { level: "warn", text: "额度偏高" };
    if (m || q) return { level: "ok", text: "正常" };
    return null;
  });

  const resetText = $derived(
    monitor.snapshot?.quota?.resetAtMs
      ? fmtCountdown(monitor.snapshot.quota.resetAtMs - now)
      : "",
  );

  const heroMessage = $derived.by(() => {
    if (!monitor.cred) return "配置监控凭证后即可查看额度";
    if (monitor.snapshot?.badCredentials) return "账号或密码错误，请更新凭证";
    if (monitor.snapshot && !monitor.snapshot.orgId) return "该账号下没有拼车套餐";
    if (monitor.snapshot?.error) return "额度获取失败，稍后重试";
    return "加载中…";
  });
</script>

<!-- ======== HERO：当前账号 ======== -->
<section class="hero">
  {#if !monitor.currentEmail}
    <div class="hero-empty">
      <LogIn size={26} />
      <div class="he-title">未检测到当前账号</div>
      <div class="he-sub">请先用 reclaude 登录，或从下方切换到某个档案。</div>
    </div>
  {:else}
    <div class="hero-head">
      <div class="who">
        <div class="who-name">{currentName ?? monitor.currentEmail}</div>
        {#if currentName}<div class="who-mail">{monitor.currentEmail}</div>{/if}
      </div>
      {#if heroBadge}
        <span class="badge {heroBadge.level}"><span class="bdot"></span>{heroBadge.text}</span>
      {/if}
    </div>

    {#if monitor.snapshot?.quota}
      {@const q = monitor.snapshot.quota}
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
      {#if monitor.snapshot.metrics}
        {@const m = monitor.snapshot.metrics}
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
        {#if !monitor.cred}
          <button class="cta" onclick={openCredsForCurrent}>
            <KeyRound size={15} /> 配置监控凭证
          </button>
        {/if}
      </div>
    {/if}
  {/if}
</section>

<style>
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
</style>
