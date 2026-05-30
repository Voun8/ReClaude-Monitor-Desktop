<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { api, type UsageStats } from "$lib/api";
  import { fmtInt, fmtTokens, fmtUsd4 } from "$lib/format";
  import { X, Maximize2 } from "@lucide/svelte";

  let stats = $state<UsageStats | null>(null);
  let status = $state<"loading" | "ok" | "nocred" | "error">("loading");
  let timer: ReturnType<typeof setInterval>;

  const today = (() => {
    const d = new Date();
    const m = String(d.getMonth() + 1).padStart(2, "0");
    const day = String(d.getDate()).padStart(2, "0");
    return `${d.getFullYear()}-${m}-${day}`;
  })();

  async function load() {
    try {
      const email = await api.currentAccount();
      if (!email) {
        status = "nocred";
        return;
      }
      const cred = await api.getMonitorCred(email);
      if (!cred) {
        status = "nocred";
        return;
      }
      stats = await api.usageStats(cred.email, cred.password, "24h", null, cred.orgId);
      status = "ok";
    } catch {
      status = stats ? "ok" : "error";
    }
  }

  function close() {
    api.hideFloat();
  }
  function openMain() {
    api.showMain();
  }

  onMount(() => {
    load();
    timer = setInterval(load, 60_000);
  });
  onDestroy(() => clearInterval(timer));
</script>

<div class="ball" data-tauri-drag-region>
  <div class="content">
    <div class="date">{today}</div>
    {#if status === "ok" && stats}
      {@const o = stats.overview}
      <div class="row"><span class="l">消息数</span><span class="v">{fmtInt(o.messages)}</span></div>
      <div class="row"><span class="l">总 Tokens</span><span class="v">{fmtTokens(o.totalTokens)}</span></div>
      <div class="row"><span class="l">总用量</span><span class="v">{fmtUsd4(o.totalUsd)}</span></div>
    {:else if status === "loading"}
      <div class="hint">加载中…</div>
    {:else if status === "nocred"}
      <div class="hint">未配置监控凭证</div>
    {:else}
      <div class="hint">获取失败</div>
    {/if}
  </div>

  <div class="btns">
    <button onclick={openMain} title="打开主面板" aria-label="打开主面板"><Maximize2 size={12} /></button>
    <button onclick={close} title="隐藏" aria-label="隐藏"><X size={13} /></button>
  </div>
</div>

<style>
  :global(html),
  :global(body) {
    background: transparent !important;
  }
  .ball {
    position: relative;
    width: 100vw;
    height: 100vh;
    box-sizing: border-box;
    padding: 12px 14px;
    border-radius: 14px;
    background: rgba(22, 23, 27, 0.94);
    border: 1px solid rgba(255, 255, 255, 0.08);
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.45);
    color: #e9eaed;
    backdrop-filter: blur(8px);
    overflow: hidden;
    cursor: grab;
  }
  .ball:active {
    cursor: grabbing;
  }
  .content {
    pointer-events: none;
  }
  .date {
    font-size: 14px;
    font-weight: 800;
    letter-spacing: 0.01em;
    margin-bottom: 9px;
  }
  .row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 8px;
    padding: 2px 0;
  }
  .l {
    font-size: 12.5px;
    color: #9aa0ab;
  }
  .v {
    font-size: 14px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }
  .hint {
    font-size: 12.5px;
    color: #9aa0ab;
    padding: 14px 0;
    text-align: center;
  }
  .btns {
    position: absolute;
    top: 7px;
    right: 7px;
    display: flex;
    gap: 3px;
    pointer-events: auto;
  }
  .btns button {
    width: 20px;
    height: 20px;
    display: grid;
    place-items: center;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: #8a8f99;
    cursor: pointer;
  }
  .btns button:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #fff;
  }
</style>
