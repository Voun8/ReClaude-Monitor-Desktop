<script lang="ts">
  // 顶部加载进度条（浏览器风格）：不确定态滑动，加载时出现、完成后淡出。
  // 纯 transform/opacity 动画，GPU 合成，零运行时开销。
  import { fade } from "svelte/transition";
  let { active = false }: { active?: boolean } = $props();
</script>

{#if active}
  <div class="loadbar" in:fade={{ duration: 100 }} out:fade={{ duration: 240 }}>
    <div class="ind"></div>
  </div>
{/if}

<style>
  .loadbar {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    height: 2.5px;
    overflow: hidden;
    z-index: 60;
    background: var(--accent-soft);
  }
  .ind {
    position: absolute;
    top: 0;
    height: 100%;
    width: 35%;
    border-radius: 999px;
    background: var(--accent);
    animation: slide 1.05s ease-in-out infinite;
    will-change: transform;
  }
  @keyframes slide {
    0% {
      transform: translateX(-120%);
    }
    100% {
      transform: translateX(385%);
    }
  }
</style>
