<script lang="ts">
  import type { QuotaOut } from "$lib/api";
  import { fmtUsd, quotaColor } from "$lib/format";
  import { ArrowLeftRight, KeyRound, Trash2 } from "@lucide/svelte";

  let {
    name,
    email,
    isActive,
    hasCreds,
    quota,
    loading,
    errorText,
    busy,
    onUse,
    onConfig,
    onRemove,
  }: {
    name: string;
    email: string;
    isActive: boolean;
    hasCreds: boolean;
    quota: QuotaOut | null;
    loading: boolean;
    errorText: string | null;
    busy: boolean;
    onUse: () => void;
    onConfig: () => void;
    onRemove: () => void;
  } = $props();

  const ratio = $derived(quota ? Math.max(0, Math.min(1, quota.ratio)) : 0);
</script>

<div class="row" class:active={isActive}>
  <div class="main">
    <div class="title-line">
      <span class="name">{name}</span>
      {#if isActive}<span class="cur"><span class="dot"></span>当前</span>{/if}
    </div>
    <div class="email">{email}</div>

    {#if hasCreds && quota}
      <div class="quota">
        <div class="bar"><div class="fill" style="width:{(ratio * 100).toFixed(1)}%;background:{quotaColor(ratio)}"></div></div>
        <span class="pct" style="color:{quotaColor(ratio)}">{quota.pct.toFixed(0)}%</span>
        <span class="usd">{fmtUsd(quota.usedUsd)} / {fmtUsd(quota.totalUsd)}</span>
      </div>
    {:else if hasCreds && loading}
      <div class="chip loading">额度加载中…</div>
    {:else if hasCreds && errorText}
      <div class="chip warn">{errorText}</div>
    {:else if !hasCreds}
      <div class="chip muted">未配置监控</div>
    {:else}
      <div class="chip muted">无额度数据</div>
    {/if}
  </div>

  <div class="ops">
    {#if !isActive}
      <button class="switch" disabled={busy} onclick={onUse}>
        <ArrowLeftRight size={15} />
        <span>{busy ? "切换中" : "切换"}</span>
      </button>
    {/if}
    <button class="icon" title="监控凭证" aria-label="监控凭证" onclick={onConfig}>
      <KeyRound size={16} />
    </button>
    <button class="icon danger" title="删除档案" aria-label="删除档案" onclick={onRemove}>
      <Trash2 size={16} />
    </button>
  </div>
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 13px 15px;
    border: 1px solid var(--border);
    border-radius: 15px;
    background: var(--surface);
    transition: border-color 0.18s ease, transform 0.06s ease;
  }
  .row:hover {
    border-color: var(--border-strong);
  }
  .row.active {
    border-color: var(--accent);
    background: linear-gradient(180deg, var(--accent-soft), transparent);
  }
  .main {
    min-width: 0;
    flex: 1;
  }
  .title-line {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .name {
    font-size: 14.5px;
    font-weight: 700;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cur {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 11px;
    font-weight: 700;
    color: var(--accent);
    background: var(--accent-soft);
    padding: 2px 8px;
    border-radius: 999px;
    flex-shrink: 0;
  }
  .cur .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
  }
  .email {
    font-size: 12px;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    margin: 2px 0 9px;
  }
  .quota {
    display: flex;
    align-items: center;
    gap: 9px;
  }
  .bar {
    flex: 1;
    min-width: 40px;
    height: 7px;
    border-radius: 999px;
    background: var(--track);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    border-radius: 999px;
    transition: width 0.5s ease;
  }
  .pct {
    font-size: 13px;
    font-weight: 800;
    min-width: 34px;
    text-align: right;
  }
  .usd {
    font-size: 11.5px;
    color: var(--faint);
    white-space: nowrap;
  }
  .chip {
    display: inline-block;
    font-size: 11.5px;
    padding: 3px 9px;
    border-radius: 999px;
    font-weight: 600;
  }
  .chip.muted {
    background: var(--surface-2);
    color: var(--faint);
  }
  .chip.loading {
    background: var(--surface-2);
    color: var(--muted);
  }
  .chip.warn {
    background: var(--warn-soft);
    color: var(--warn);
  }
  .ops {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }
  .switch {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 13px;
    border-radius: 10px;
    border: none;
    background: var(--accent);
    color: #fff;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    transition: filter 0.15s ease;
  }
  .switch:hover:not(:disabled) {
    filter: brightness(1.08);
  }
  .switch:disabled {
    opacity: 0.55;
    cursor: default;
  }
  .icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 34px;
    border-radius: 10px;
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--muted);
    cursor: pointer;
    transition: all 0.15s ease;
  }
  .icon:hover {
    color: var(--fg);
    border-color: var(--border-strong);
  }
  .icon.danger:hover {
    color: var(--err);
    border-color: var(--err);
    background: var(--err-soft);
  }
</style>
