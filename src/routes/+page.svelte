<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import FloatWidget from "$lib/components/FloatWidget.svelte";
  import PanelWidget from "$lib/components/PanelWidget.svelte";
  import MainPanel from "$lib/components/MainPanel.svelte";

  // 悬浮球 / 托盘面板 / 主面板共用本路由，按 Tauri 窗口 label 分流
  function detectWindow(): "float" | "panel" | "main" {
    try {
      const l = getCurrentWindow().label;
      if (l === "float") return "float";
      if (l === "panel") return "panel";
      return "main";
    } catch {
      return "main";
    }
  }
  const win = detectWindow();
</script>

{#if win === "float"}
  <FloatWidget />
{:else if win === "panel"}
  <PanelWidget />
{:else}
  <MainPanel />
{/if}
