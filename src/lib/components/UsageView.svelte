<script lang="ts">
  import { api, type Device, type MonitorCred, type UsageStats } from "$lib/api";
  import { fmtInt, fmtTokens, fmtUsd4 } from "$lib/format";
  import Heatmap from "./Heatmap.svelte";
  import ActivityBars from "./ActivityBars.svelte";
  import { RefreshCw, RotateCw, KeyRound, BarChart3, CalendarDays } from "@lucide/svelte";

  let {
    cred,
    onConfigure,
  }: { cred: MonitorCred | null; onConfigure: () => void } = $props();

  type Range = "24h" | "7d" | "30d" | "all";
  let range = $state<Range>("24h");
  let pickedDate = $state(""); // YYYY-MM-DD，选某天
  let deviceId = $state("");
  let devices = $state<Device[]>([]);
  let stats = $state<UsageStats | null>(null);
  let loading = $state(false);
  let syncing = $state(false);
  let error = $state<string | null>(null);

  const RANGES: { v: Range; t: string }[] = [
    { v: "24h", t: "24 小时" },
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

  function pct(p: number): number {
    const v = p <= 1 ? p * 100 : p;
    return Math.max(0, Math.min(100, v));
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

  // 图表跟随上方：选了某天 → 只看那天；否则按 range 取时间窗
  const windowed = $derived.by(() => {
    if (pickedDate) return sorted.filter((p) => p.date.slice(0, 10) === pickedDate);
    if (range === "all" || sorted.length === 0) return [];
    const win = range === "24h" ? 864e5 : range === "7d" ? 7 * 864e5 : 30 * 864e5;
    const maxTs = sorted[sorted.length - 1].ts;
    return sorted.filter((p) => p.ts > maxTs - win);
  });

  const showHeatmap = $derived(!pickedDate && range === "all");
  const chartTitle = $derived(
    pickedDate
      ? `${pickedDate} · 活动`
      : range === "all"
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
</script>

{#if !cred}
  <div class="need-cred">
    <KeyRound size={24} />
    <div class="nc-title">需要监控凭证</div>
    <div class="nc-sub">用量统计要登录 reclaude.ai，请先为当前账号配置邮箱密码。</div>
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
      <button class="sync" disabled={syncing} onclick={doSync} title="向 reclaude 重新拉取并重算用量">
        <RotateCw size={14} class={syncing ? "spin" : ""} />
        <span>{syncing ? "同步中…" : "同步"}</span>
      </button>
      <button class="ref" class:spin={loading} onclick={loadStats} title="刷新" aria-label="刷新">
        <RefreshCw size={15} />
      </button>
    </div>
  </div>

  <div class="controls">
    <div class="seg">
      {#each RANGES as r (r.v)}
        <button class:on={range === r.v && !pickedDate} onclick={() => { pickedDate = ""; range = r.v; }}>{r.t}</button>
      {/each}
    </div>
    <label class="date" class:active={!!pickedDate}>
      <CalendarDays size={14} />
      <input type="date" bind:value={pickedDate} aria-label="选择日期" />
      {#if pickedDate}<button class="clr" onclick={() => (pickedDate = "")} aria-label="清除日期">✕</button>{/if}
    </label>
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
        <ActivityBars points={windowed} mode={range === "24h" && !pickedDate ? "hour" : "day"} />
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
    max-width: 46%;
    padding: 8px 10px;
    border-radius: 10px;
    background: var(--surface);
    color: var(--fg);
    border: 1px solid var(--border-strong);
    font-size: 12.5px;
    outline: none;
    cursor: pointer;
  }
  .dev:focus {
    border-color: var(--accent);
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
  .ref {
    width: 34px;
    height: 34px;
    border-radius: 10px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--muted);
    cursor: pointer;
    display: grid;
    place-items: center;
  }
  .ref:hover {
    color: var(--accent);
    border-color: var(--accent);
  }
  :global(.spin) {
    animation: spin 0.9s linear infinite;
  }
  .ref.spin :global(svg) {
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
  .date {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 9px;
    border-radius: 10px;
    border: 1px solid var(--border-strong);
    background: var(--surface);
    color: var(--muted);
    font-size: 12px;
  }
  .date.active {
    border-color: var(--accent);
    color: var(--accent);
  }
  .date input {
    border: none;
    background: transparent;
    color: var(--fg);
    font-size: 12px;
    outline: none;
    width: 116px;
    color-scheme: dark light;
  }
  .clr {
    border: none;
    background: transparent;
    color: var(--muted);
    cursor: pointer;
    font-size: 11px;
    padding: 0 2px;
  }
  .clr:hover {
    color: var(--err);
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
