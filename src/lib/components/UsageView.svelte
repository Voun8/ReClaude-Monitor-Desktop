<script lang="ts">
  import { api, type Device, type MonitorCred, type UsageStats } from "$lib/api";
  import { fmtInt, fmtTokens, fmtUsd4 } from "$lib/format";
  import Heatmap from "./Heatmap.svelte";
  import ActivityBars from "./ActivityBars.svelte";
  import { RotateCw, KeyRound, BarChart3 } from "@lucide/svelte";

  let {
    cred,
    reloadKey = 0,
    onConfigure,
  }: {
    cred: MonitorCred | null;
    reloadKey?: number;
    onConfigure: () => void;
  } = $props();

  type Range = "7d" | "30d" | "all";
  let range = $state<Range>("7d");
  let deviceId = $state("");
  let devices = $state<Device[]>([]);
  let stats = $state<UsageStats | null>(null);
  let loading = $state(false);
  let syncing = $state(false);
  let error = $state<string | null>(null);

  const RANGES: { v: Range; t: string }[] = [
    { v: "7d", t: "7 天" },
    { v: "30d", t: "30 天" },
    { v: "all", t: "全部" },
  ];

  const kpis = $derived.by(() => {
    const o = stats?.overview;
    if (!o) return [];
    return [
      { label: "会话数", value: fmtInt(o.sessions) },
      { label: "消息数", value: fmtInt(o.messages) },
      { label: "总用量", value: fmtUsd4(o.totalUsd) },
      { label: "总 Tokens", value: fmtTokens(o.totalTokens) },
      { label: "活跃天数", value: fmtInt(o.activeDays) },
      { label: "当前连续", value: `${Math.round(o.currentStreak)}d` },
      { label: "最长连续", value: `${Math.round(o.longestStreak)}d` },
      { label: "常用模型", value: o.favoriteModel || "—" },
    ];
  });

  // 后端已统一为 0-100，前端只做钳制
  function pct(p: number): number {
    if (!Number.isFinite(p)) return 0;
    return Math.max(0, Math.min(100, p));
  }

  function tsOf(s: string): number {
    if (/^\d{4}-\d{2}-\d{2}/.test(s)) return new Date(s.slice(0, 19)).getTime();
    const n = Number(s);
    if (s.trim() !== "" && !Number.isNaN(n)) return n < 1e12 ? n * 1000 : n;
    const d = new Date(s);
    return Number.isNaN(d.getTime()) ? 0 : d.getTime();
  }

  const sorted = $derived(
    (stats?.heatmap ?? [])
      .map((c) => ({ date: c.date, value: c.count, ts: tsOf(c.date) }))
      .filter((p) => p.ts > 0)
      .sort((a, b) => a.ts - b.ts),
  );

  // 图表按 range 取时间窗
  const windowed = $derived.by(() => {
    if (range === "all" || sorted.length === 0) return [];
    const win = range === "7d" ? 7 * 864e5 : 30 * 864e5;
    const maxTs = sorted[sorted.length - 1].ts;
    return sorted.filter((p) => p.ts > maxTs - win);
  });

  const showHeatmap = $derived(range === "all");
  const chartTitle = $derived(
    range === "all"
      ? "活动 · 近一年"
      : `活动趋势 · ${RANGES.find((r) => r.v === range)?.t ?? ""}`,
  );

  async function loadDevices() {
    if (!cred) return;
    try {
      devices = await api.usageDevices(cred.email, cred.password);
    } catch {
      devices = [];
    }
  }

  async function loadStats() {
    if (!cred) {
      stats = null;
      return;
    }
    loading = true;
    error = null;
    try {
      stats = await api.usageStats(cred.email, cred.password, range, deviceId || null, cred.orgId);
    } catch (e) {
      error = String(e);
      stats = null;
    } finally {
      loading = false;
    }
  }

  async function doSync() {
    if (!cred || syncing) return;
    syncing = true;
    error = null;
    try {
      await api.usageSync(cred.email, cred.password);
      await loadStats();
    } catch (e) {
      error = String(e);
    } finally {
      syncing = false;
    }
  }

  let devKey = "";
  $effect(() => {
    const k = cred ? cred.email : "";
    if (k !== devKey) {
      devKey = k;
      loadDevices();
    }
  });

  // cred / range / device 变化 → 重载（KPI 与图表都跟随）
  let statKey = "";
  $effect(() => {
    const k = cred ? `${cred.email}|${range}|${deviceId}` : "";
    if (k !== statKey) {
      statKey = k;
      loadStats();
    }
  });

  // 父级头部刷新按钮触发（reloadKey 递增）→ 重新加载用量（跳过首次挂载）
  let reloadSeen = false;
  $effect(() => {
    void reloadKey; // 跟踪 reloadKey 变化
    if (!reloadSeen) {
      reloadSeen = true;
      return;
    }
    loadStats();
  });
</script>

{#if !cred}
  <div class="need-cred">
    <KeyRound size={24} />
    <div class="nc-title">需要监控凭证</div>
    <div class="nc-sub">用量统计需要登录 API 服务，请先为当前账号配置邮箱密码。</div>
    <button class="cta" onclick={onConfigure}><KeyRound size={15} /> 配置监控凭证</button>
  </div>
{:else}
  <div class="toolbar">
    <select class="dev" bind:value={deviceId} aria-label="设备">
      <option value="">全部设备</option>
      {#each devices as d (d.id)}
        <option value={d.id}>{d.name}</option>
      {/each}
    </select>
    <div class="right">
      <button class="sync" disabled={syncing} onclick={doSync} title="向 reclaude 服务器重新拉取并重算用量（较慢）">
        <RotateCw size={14} class={syncing ? "spin" : ""} />
        <span>{syncing ? "重算中…" : "重算"}</span>
      </button>
    </div>
  </div>

  <div class="controls">
    <div class="seg">
      {#each RANGES as r (r.v)}
        <button class:on={range === r.v} onclick={() => (range = r.v)}>{r.t}</button>
      {/each}
    </div>
  </div>

  {#if error}
    <div class="msg err">{error}</div>
  {:else if !stats && loading}
    <div class="msg">加载中…</div>
  {:else if stats}
    <div class="kpis">
      {#each kpis as k}
        <div class="kpi">
          <div class="kl">{k.label}</div>
          <div class="kv">{k.value}</div>
        </div>
      {/each}
    </div>

    <div class="card">
      <div class="card-h">{chartTitle}</div>
      {#if showHeatmap}
        {#if stats.heatmap.length > 0}
          <Heatmap cells={stats.heatmap} />
        {:else}
          <div class="muted">暂无活动数据</div>
        {/if}
      {:else}
        <ActivityBars points={windowed} />
      {/if}
    </div>

    {#if stats.models.length > 0}
      <div class="card">
        <div class="card-h"><BarChart3 size={14} /> 按模型</div>
        <div class="models">
          {#each stats.models as m (m.model)}
            <div class="mrow">
              <div class="mtop">
                <span class="mname">{m.model || "—"}</span>
                <span class="mcost">{fmtUsd4(m.totalUsd)}</span>
              </div>
              <div class="mbar"><div class="mfill" style="width:{pct(m.percent)}%"></div></div>
              <div class="msub">{fmtTokens(m.totalTokens)} tokens · {pct(m.percent).toFixed(0)}%</div>
            </div>
          {/each}
        </div>
      </div>
    {/if}
  {:else}
    <div class="msg">暂无数据</div>
  {/if}
{/if}

<style>
  .need-cred {
    text-align: center;
    padding: 40px 16px;
    color: var(--muted);
  }
  .need-cred :global(svg) {
    color: var(--faint);
  }
  .nc-title {
    font-size: 15px;
    font-weight: 700;
    color: var(--fg);
    margin: 10px 0 4px;
  }
  .nc-sub {
    font-size: 12.5px;
    line-height: 1.6;
    margin-bottom: 16px;
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

  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-bottom: 10px;
  }
  .dev {
    appearance: none;
    -webkit-appearance: none;
    max-width: 52%;
    padding: 9px 32px 9px 13px;
    border-radius: 11px;
    background-color: var(--surface);
    color: var(--fg);
    border: 1px solid var(--border-strong);
    font-size: 12.5px;
    font-weight: 600;
    outline: none;
    cursor: pointer;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='14' height='14' viewBox='0 0 24 24' fill='none' stroke='%239aa0ab' stroke-width='2.5' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpolyline points='6 9 12 15 18 9'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right 11px center;
    transition:
      border-color 0.15s ease,
      background-color 0.15s ease,
      box-shadow 0.15s ease;
  }
  .dev:hover {
    border-color: var(--accent);
    background-color: var(--surface-2);
  }
  .dev:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }
  .dev option {
    background: var(--surface);
    color: var(--fg);
  }
  .right {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .sync {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 12px;
    border-radius: 10px;
    border: 1px solid var(--border-strong);
    background: var(--surface);
    color: var(--fg);
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
  }
  .sync:hover:not(:disabled) {
    border-color: var(--accent);
    color: var(--accent);
  }
  .sync:disabled {
    opacity: 0.6;
    cursor: default;
  }
  :global(.spin) {
    animation: spin 0.9s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .controls {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-bottom: 14px;
  }
  .seg {
    display: inline-flex;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 2px;
  }
  .seg button {
    border: none;
    background: transparent;
    color: var(--muted);
    font-size: 12px;
    font-weight: 600;
    padding: 6px 9px;
    border-radius: 8px;
    cursor: pointer;
  }
  .seg button.on {
    background: var(--accent);
    color: #fff;
  }
  .msg {
    text-align: center;
    color: var(--muted);
    padding: 30px;
    font-size: 13px;
  }
  .msg.err {
    color: var(--err);
  }
  .kpis {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 9px;
    margin-bottom: 14px;
  }
  .kpi {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 13px;
    padding: 12px 14px;
  }
  .kl {
    font-size: 12px;
    color: var(--muted);
    margin-bottom: 5px;
  }
  .kv {
    font-size: 20px;
    font-weight: 800;
    letter-spacing: -0.01em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 15px;
    padding: 15px;
    margin-bottom: 14px;
  }
  .card-h {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    font-weight: 700;
    color: var(--muted);
    margin-bottom: 12px;
  }
  .muted {
    color: var(--faint);
    font-size: 12.5px;
    text-align: center;
    padding: 12px;
  }
  .models {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .mrow {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .mtop {
    display: flex;
    justify-content: space-between;
    gap: 8px;
  }
  .mname {
    font-size: 13px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .mcost {
    font-size: 13px;
    font-weight: 700;
  }
  .mbar {
    height: 7px;
    border-radius: 999px;
    background: var(--track);
    overflow: hidden;
  }
  .mfill {
    height: 100%;
    border-radius: 999px;
    background: var(--accent);
  }
  .msub {
    font-size: 11.5px;
    color: var(--faint);
  }
</style>
