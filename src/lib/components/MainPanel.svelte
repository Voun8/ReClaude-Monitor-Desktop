<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { AlertTriangle, PictureInPicture2, RefreshCw, Settings } from "@lucide/svelte";
  import { api } from "$lib/api";
  import { monitor, refreshAll, follow, doRefresh, fetchAccountQuotas, loadEnv } from "$lib/monitor.svelte";
  import {
    settings,
    setView,
    initFromBackend,
    applyTrayMode,
    enterFloat,
    REFRESH_SEC_MIN,
  } from "$lib/settings.svelte";
  import { ui, openModal, closeModal } from "$lib/ui.svelte";
  import Logo from "./Logo.svelte";
  import MonitorView from "./MonitorView.svelte";
  import UsageView from "./UsageView.svelte";
  import Toasts from "./Toasts.svelte";
  import SaveProfileModal from "./modals/SaveProfileModal.svelte";
  import CredsModal from "./modals/CredsModal.svelte";
  import UseProfileModal from "./modals/UseProfileModal.svelte";
  import RemoveProfileModal from "./modals/RemoveProfileModal.svelte";
  import SettingsModal from "./modals/SettingsModal.svelte";

  const FOLLOW_MS = 10_000;

  let usageReloadKey = $state(0); // 递增以触发用量页重新加载

  // 头部刷新：按当前页刷新——监控页刷新额度/服务，用量页重载用量
  function headerRefresh() {
    if (settings.view === "usage") usageReloadKey++;
    else refreshAll();
  }

  function openCredsForCurrent() {
    openModal({ kind: "creds", profileName: null, email: monitor.currentEmail ?? "" });
  }

  function onSettingsSaved(apiChanged: boolean) {
    if (apiChanged) {
      usageReloadKey++;
      void refreshAll();
    }
  }

  let followTimer: ReturnType<typeof setInterval>;
  let unlistenClose: (() => void) | undefined;

  onMount(async () => {
    await initFromBackend();
    applyTrayMode();
    // 圆环模式：本次挂载是 Rust 为渲染圆环而显示的主窗口 → 渲染后隐藏主窗口。
    // 悬浮球模式：球已由 Rust 显示，主窗口全程隐藏，这里只在用户点开主窗口时运行，不隐藏。
    if (settings.floatMode === "tray") {
      await api.hideMain();
    }

    await loadEnv();
    await refreshAll();
    followTimer = setInterval(follow, FOLLOW_MS);

    // 关闭按钮：退出程序 或 后台运行。
    // Svelte 5 注：closure 内 settings.closeAction 是动态读取（$state 编译为 proxy getter）——
    // 用户切了设置立刻生效，无需重绑 listener。
    try {
      unlistenClose = await getCurrentWindow().onCloseRequested((event) => {
        if (settings.closeAction === "background") {
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
    clearInterval(followTimer);
    unlistenClose?.();
  });

  // 额度轮询定时器：随 refreshSec 变化重建
  $effect(() => {
    const ms = Math.max(REFRESH_SEC_MIN, settings.refreshSec) * 1000;
    const t = setInterval(() => {
      doRefresh();
      fetchAccountQuotas();
    }, ms);
    return () => clearInterval(t);
  });
</script>

<div class="app">
  <header>
    <div class="brand">
      <div class="logo"><Logo size={38} /></div>
      <div class="t">Reclaude 控制台</div>
    </div>
    <div class="head-actions">
      <button class="iconbtn" onclick={() => openModal({ kind: "settings" })} title="设置刷新间隔" aria-label="设置">
        <Settings size={17} />
      </button>
      <button
        class="iconbtn"
        onclick={enterFloat}
        title={settings.floatMode === "tray" ? "最小化到菜单栏圆环" : "最小化为悬浮球"}
        aria-label="最小化"
      >
        <PictureInPicture2 size={17} />
      </button>
      <button class="refresh" class:spin={monitor.refreshing && settings.view !== "usage"} onclick={headerRefresh} title="刷新" aria-label="刷新">
        <RefreshCw size={17} />
      </button>
    </div>
  </header>

  {#if monitor.env && !monitor.env.reclaudeFound}
    <div class="banner">
      <AlertTriangle size={15} />
      <span>未找到 reclaude，账号切换不可用（监控仍可用）。</span>
    </div>
  {/if}

  <nav class="tabs">
    <button class:on={settings.view === "monitor"} onclick={() => setView("monitor")}>监控</button>
    <button class:on={settings.view === "usage"} onclick={() => setView("usage")}>用量</button>
  </nav>

  {#if settings.view === "usage"}
    <UsageView cred={monitor.cred} reloadKey={usageReloadKey} onConfigure={openCredsForCurrent} />
  {:else}
    <MonitorView />
  {/if}
</div>

<Toasts />

{#if ui.modal?.kind === "save"}
  <SaveProfileModal onClose={closeModal} />
{:else if ui.modal?.kind === "creds"}
  <CredsModal profileName={ui.modal.profileName} email={ui.modal.email} onClose={closeModal} />
{:else if ui.modal?.kind === "use"}
  <UseProfileModal profile={ui.modal.profile} onClose={closeModal} />
{:else if ui.modal?.kind === "remove"}
  <RemoveProfileModal profile={ui.modal.profile} onClose={closeModal} />
{:else if ui.modal?.kind === "settings"}
  <SettingsModal onClose={closeModal} onSaved={onSettingsSaved} />
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
</style>
