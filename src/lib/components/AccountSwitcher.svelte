<script lang="ts">
  import { Plus } from "@lucide/svelte";
  import { monitor } from "$lib/monitor.svelte";
  import { settings, setNoApp } from "$lib/settings.svelte";
  import { openModal } from "$lib/ui.svelte";
  import AccountRow from "./AccountRow.svelte";
</script>

<!-- ======== 切换账号 ======== -->
<section class="switch">
  <div class="sec-head">
    <span class="sec-title">切换账号</span>
    <span class="count">{monitor.profiles.length}</span>
  </div>

  {#if monitor.profiles.length === 0}
    <div class="list-empty">
      还没有保存任何档案。<br />
      把当前登录的账号保存为档案，之后就能一键切回。
    </div>
  {:else}
    <div class="list">
      {#each monitor.profiles as p (p.name)}
        <AccountRow
          profile={p}
          busy={monitor.busyName === p.name}
          onUse={() => openModal({ kind: "use", profile: p })}
          onConfig={() => openModal({ kind: "creds", profileName: p.name, email: p.email })}
          onRemove={() => openModal({ kind: "remove", profile: p })}
        />
      {/each}
    </div>
  {/if}

  <button class="save-btn" disabled={!monitor.currentEmail} onclick={() => openModal({ kind: "save" })}>
    <Plus size={17} /> 保存当前账号为档案
  </button>
  <p class="save-hint">把当前登录账号的身份与桌面会话整套快照下来，方便随时切回。</p>
</section>

<footer>
  <label class="toggle">
    <input
      type="checkbox"
      checked={settings.noApp}
      onchange={(e) => setNoApp(e.currentTarget.checked)}
    />
    <span>切换时只换凭证，不自动打开桌面 App</span>
  </label>
  {#if monitor.lastUpdated}
    <span class="ts">更新于 {new Date(monitor.lastUpdated).toLocaleTimeString("zh-CN")}</span>
  {/if}
</footer>

<style>
  /* switch section */
  .sec-head {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 4px 11px;
  }
  .sec-title {
    font-size: 14px;
    font-weight: 700;
    color: var(--muted);
  }
  .count {
    font-size: 11.5px;
    font-weight: 700;
    color: var(--faint);
    background: var(--surface-2);
    border-radius: 999px;
    padding: 1px 9px;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 9px;
  }
  .list-empty {
    color: var(--muted);
    font-size: 12.5px;
    line-height: 1.8;
    text-align: center;
    padding: 14px;
    border: 1px dashed var(--border-strong);
    border-radius: 15px;
  }
  .save-btn {
    width: 100%;
    margin-top: 12px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 12px;
    border-radius: 14px;
    border: 1px dashed var(--border-strong);
    background: transparent;
    color: var(--fg);
    font-size: 13.5px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.15s ease;
  }
  .save-btn:hover:not(:disabled) {
    border-color: var(--accent);
    color: var(--accent);
    background: var(--accent-soft);
  }
  .save-btn:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .save-hint {
    font-size: 11.5px;
    color: var(--faint);
    text-align: center;
    margin: 9px 6px 0;
    line-height: 1.6;
  }
  footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-top: 20px;
    padding-top: 14px;
    border-top: 1px solid var(--border);
  }
  .toggle {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--muted);
    cursor: pointer;
  }
  .toggle input {
    width: 15px;
    height: 15px;
    accent-color: var(--accent);
  }
  .ts {
    font-size: 11px;
    color: var(--faint);
    white-space: nowrap;
  }
</style>
