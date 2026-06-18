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

<svelte:window onkeydown={onKey} />

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
    /* 固定窗口大小：弹窗最高不超过视口（减去 overlay 上下各 20px 内边距），
       超高时由 .body 内部滚动，而不是把弹窗撑出窗口、裁掉首尾（含保存按钮）。 */
    max-height: calc(100vh - 40px);
    display: flex;
    flex-direction: column;
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
    flex-shrink: 0; /* 头部固定，标题不随 body 滚动 */
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
    overflow-y: auto; /* 内容超高时滚动，配合 .dialog 的 max-height 固定窗口 */
    scrollbar-gutter: stable; /* 始终预留滚动条槽位，内容高度变化时不横跳 */
    overscroll-behavior: contain;
  }

  /* 弹窗表单的公共词汇表（field / cbox / modal-foot / hint / confirm），
     供各业务弹窗的 children 内容复用，避免每个弹窗各抄一份 */
  .body :global(.field) {
    margin-bottom: 11px;
  }
  .body :global(.field label) {
    display: block;
    font-size: 12px;
    color: var(--muted);
    margin-bottom: 5px;
  }
  .body :global(.field input) {
    width: 100%;
    padding: 9px 11px;
    font-size: 13px;
    border-radius: 9px;
    background: var(--surface-2);
    color: var(--fg);
    border: 1px solid var(--border-strong);
    outline: none;
  }
  .body :global(.field input:focus) {
    border-color: var(--accent);
  }
  .body :global(.cbox) {
    display: flex;
    align-items: center;
    gap: 9px;
    font-size: 12.5px;
    margin: 4px 0 12px;
    cursor: pointer;
  }
  .body :global(.cbox input) {
    width: 15px;
    height: 15px;
    accent-color: var(--accent);
  }
  .body :global(.modal-foot) {
    display: flex;
    gap: 8px;
    margin-top: 6px;
  }
  .body :global(.modal-foot button) {
    padding: 9px 16px;
    border-radius: 9px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    border: 1px solid transparent;
  }
  .body :global(.primary) {
    background: var(--accent);
    color: #fff;
  }
  .body :global(.primary:hover:not(:disabled)) {
    filter: brightness(1.08);
  }
  .body :global(.primary:disabled) {
    opacity: 0.6;
    cursor: default;
  }
  .body :global(.danger) {
    background: var(--err);
    color: #fff;
  }
  .body :global(.danger:hover:not(:disabled)) {
    filter: brightness(1.05);
  }
  .body :global(.cancel) {
    background: var(--surface-2);
    color: var(--fg);
    border: 1px solid var(--border-strong);
  }
  .body :global(.cancel:hover) {
    border-color: var(--muted);
  }
  .body :global(.hint) {
    font-size: 11.5px;
    color: var(--faint);
    margin: 12px 0 0;
    line-height: 1.6;
  }
  .body :global(.confirm) {
    font-size: 14px;
    margin: 0 0 4px;
    line-height: 1.6;
  }
  .body :global(.cmuted) {
    color: var(--muted);
    font-size: 12.5px;
  }
</style>
