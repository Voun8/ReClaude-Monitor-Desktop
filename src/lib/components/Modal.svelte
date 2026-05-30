<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    title,
    onClose,
    children,
  }: { title: string; onClose: () => void; children: Snippet } = $props();

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") onClose();
  }
</script>

<svelte:window on:keydown={onKey} />

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="overlay" onclick={onClose} role="presentation">
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="dialog"
    onclick={(e) => e.stopPropagation()}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
  >
    <div class="head">
      <span class="title">{title}</span>
      <button class="x" onclick={onClose} aria-label="关闭">&#10005;</button>
    </div>
    <div class="body">
      {@render children()}
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    backdrop-filter: blur(2px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 50;
    padding: 20px;
  }
  .dialog {
    width: 100%;
    max-width: 380px;
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow);
    overflow: hidden;
    animation: pop 0.14s ease;
  }
  @keyframes pop {
    from {
      transform: translateY(6px) scale(0.98);
      opacity: 0;
    }
    to {
      transform: none;
      opacity: 1;
    }
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 16px;
    border-bottom: 1px solid var(--border);
  }
  .title {
    font-size: 14.5px;
    font-weight: 700;
  }
  .x {
    border: none;
    background: transparent;
    color: var(--muted);
    cursor: pointer;
    font-size: 13px;
    padding: 4px 6px;
    border-radius: 6px;
  }
  .x:hover {
    color: var(--err);
    background: var(--err-soft);
  }
  .body {
    padding: 16px;
  }
</style>
