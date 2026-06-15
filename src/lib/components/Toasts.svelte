<script lang="ts">
  import { toasts } from "$lib/ui.svelte";
</script>

<div class="toasts">
  {#each toasts as t (t.id)}
    <div class="toast {t.level}">{t.text}</div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    bottom: 18px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: 60;
    align-items: center;
  }
  .toast {
    padding: 9px 16px;
    border-radius: 11px;
    font-size: 13px;
    font-weight: 600;
    box-shadow: var(--shadow);
    background: var(--surface);
    border: 1px solid var(--border-strong);
    color: var(--fg);
    animation: rise 0.16s ease;
    max-width: 80vw;
  }
  .toast.ok {
    border-color: var(--ok);
    color: var(--ok);
  }
  .toast.err {
    border-color: var(--err);
    color: var(--err);
  }
  .toast.warn {
    border-color: var(--warn);
    color: var(--warn);
  }
  @keyframes rise {
    from {
      transform: translateY(6px);
      opacity: 0;
    }
    to {
      transform: none;
      opacity: 1;
    }
  }
</style>
