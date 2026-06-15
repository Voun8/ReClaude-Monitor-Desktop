<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import Modal from "../Modal.svelte";
  import { api, type Allocation, type MonitorCred } from "$lib/api";
  import { refreshAll } from "$lib/monitor.svelte";
  import { toast } from "$lib/ui.svelte";

  let {
    profileName,
    email,
    onClose,
  }: { profileName: string | null; email: string; onClose: () => void } = $props();

  // 弹窗每次打开都重新挂载，这里只取打开时刻的初始 email
  // svelte-ignore state_referenced_locally
  const draft = $state({
    email,
    password: "",
    orgId: "",
    allocs: null as Allocation[] | null,
    detecting: false,
    busy: false,
  });

  // 弹窗关闭后丢弃在途异步结果（探测/回填），不再弹 toast 或改 draft
  let alive = true;
  onDestroy(() => (alive = false));

  // 回填该邮箱已保存的凭证（本地文件读取，瞬时完成）
  onMount(async () => {
    if (!email) return;
    try {
      const existing = await api.getMonitorCred(email);
      // 用户已开始输入则不覆盖
      if (!alive || !existing || draft.password || draft.orgId || draft.email !== email) return;
      draft.email = existing.email;
      draft.password = existing.password;
      draft.orgId = existing.orgId;
    } catch (e) {
      console.error(e);
    }
  });

  async function detectOrg() {
    const mail = draft.email.trim();
    if (!mail || !draft.password) {
      toast("请先填写邮箱和密码", "err");
      return;
    }
    draft.detecting = true;
    try {
      const list = await api.listAllocations(mail, draft.password);
      if (!alive) return;
      if (list.length === 0) {
        toast("该账号下没有拼车套餐", "warn");
        draft.allocs = null;
      } else if (list.length === 1) {
        draft.orgId = list[0].id;
        draft.allocs = null;
        toast(`已探测到组织 ID ${list[0].id}`);
      } else {
        draft.allocs = list;
      }
    } catch (e) {
      toast(String(e), "err");
    } finally {
      if (alive) draft.detecting = false;
    }
  }

  function pickAlloc(a: Allocation) {
    draft.orgId = a.id;
    draft.allocs = null;
  }

  async function confirm() {
    const mail = draft.email.trim();
    if (!mail || !draft.password) {
      toast("请填写邮箱和密码", "err");
      return;
    }
    draft.busy = true;
    const c: MonitorCred = { email: mail, password: draft.password, orgId: draft.orgId.trim() };
    try {
      await api.setMonitorCred(c, profileName);
      toast("监控凭证已保存");
      onClose();
      await refreshAll();
    } catch (e) {
      toast(String(e), "err");
      draft.busy = false;
    }
  }
</script>

<Modal title="监控凭证" {onClose}>
  <div class="field"><label for="c-email">邮箱</label><input id="c-email" type="text" bind:value={draft.email} /></div>
  <div class="field"><label for="c-pass">密码</label><input id="c-pass" type="password" bind:value={draft.password} /></div>
  <div class="field">
    <label for="c-org">组织 ID</label>
    <div class="org-row">
      <input id="c-org" type="text" placeholder="留空将自动探测" bind:value={draft.orgId} />
      <button class="detect" disabled={draft.detecting} onclick={detectOrg}>{draft.detecting ? "探测中…" : "自动探测"}</button>
    </div>
  </div>
  {#if draft.allocs}
    <div class="alloc-list">
      <div class="alloc-hint">该账号有多个拼车套餐，选择一个：</div>
      {#each draft.allocs as a (a.id)}
        <button class="alloc" onclick={() => pickAlloc(a)}>
          <span>{a.label}</span>{#if a.capacity}<span class="cap">{a.capacity} 人</span>{/if}
        </button>
      {/each}
    </div>
  {/if}
  <div class="modal-foot">
    <button class="primary" disabled={draft.busy} onclick={confirm}>{draft.busy ? "保存中…" : "保存"}</button>
    <button class="cancel" onclick={onClose}>取消</button>
  </div>
  <p class="hint">{#if profileName}保存到档案 “{profileName}” 的 monitor.json{:else}保存为当前账号的监控凭证{/if}</p>
</Modal>

<style>
  .org-row {
    display: flex;
    gap: 7px;
  }
  .org-row input {
    flex: 1;
  }
  .detect {
    white-space: nowrap;
    border: 1px solid var(--border-strong);
    background: var(--surface-2);
    color: var(--fg);
    border-radius: 9px;
    padding: 0 13px;
    cursor: pointer;
    font-size: 12.5px;
  }
  .detect:hover:not(:disabled) {
    border-color: var(--accent);
    color: var(--accent);
  }
  .detect:disabled {
    opacity: 0.6;
  }
  .alloc-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-bottom: 11px;
  }
  .alloc-hint {
    font-size: 12px;
    color: var(--muted);
  }
  .alloc {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    padding: 9px 12px;
    border: 1px solid var(--border-strong);
    border-radius: 9px;
    background: var(--surface-2);
    color: var(--fg);
    cursor: pointer;
    font-size: 12.5px;
    text-align: left;
  }
  .alloc:hover {
    border-color: var(--accent);
  }
  .cap {
    color: var(--muted);
  }
</style>
