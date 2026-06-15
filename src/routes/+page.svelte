<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import FloatWidget from "$lib/components/FloatWidget.svelte";
  import MainPanel from "$lib/components/MainPanel.svelte";

  // 悬浮球窗口与主面板共用本路由，按 Tauri 窗口 label 分流
  function detectFloat(): boolean {
    try {
      return getCurrentWindow().label === "float";
    } catch {
      return false;
    }
  }
  const isFloat = detectFloat();
</script>

{#if isFloat}
  <FloatWidget />
{:else}
  <MainPanel />
{/if}
