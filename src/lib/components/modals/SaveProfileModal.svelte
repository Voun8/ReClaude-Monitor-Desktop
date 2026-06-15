<script lang="ts">
  import Modal from "../Modal.svelte";
  import { api, type MonitorCred } from "$lib/api";
  import { monitor, currentProfileName, refreshAll } from "$lib/monitor.svelte";
  import { toast } from "$lib/ui.svelte";

  let { onClose }: { onClose: () => void } = $props();

  const draft = $state({
    name: currentProfileName() ?? "",
    incMon: !monitor.cred,
    email: monitor.currentEmail ?? "",
    password: monitor.cred?.password ?? "",
    orgId: monitor.cred?.orgId ?? "",
    busy: false,
  });

  async function confirm() {
    const name = draft.name.trim();
    if (!name) {
      toast("请填写档案名", "err");
      return;
    }
    draft.busy = true;
    const mon: MonitorCred | null =
      draft.incMon && draft.email.trim() && draft.password
        ? { email: draft.email.trim(), password: draft.password, orgId: draft.orgId.trim() }
        : null;
    try {
      await api.saveProfile(name, mon);
      toast(`已保存档案 “${name}”`);
      onClose();
      await refreshAll();
    } catch (e) {
      toast(String(e), "err");
      draft.busy = false;
    }
  }
</script>

<Modal title="保存当前账号为档案" {onClose}>
  <div class="field">
    <label for="m-name">档案名</label>
    <input id="m-name" type="text" placeholder="例如 work / home" bind:value={draft.name} />
  </div>
  <label class="cbox">
    <input type="checkbox" bind:checked={draft.incMon} />
    <span>同时保存监控凭证（用于额度监控）</span>
  </label>
  {#if draft.incMon}
    <div class="field"><label for="m-email">邮箱</label><input id="m-email" type="text" bind:value={draft.email} /></div>
    <div class="field"><label for="m-pass">密码</label><input id="m-pass" type="password" bind:value={draft.password} /></div>
    <div class="field"><label for="m-org">组织 ID（可留空，自动探测）</label><input id="m-org" type="text" placeholder="例如 2440" bind:value={draft.orgId} /></div>
  {/if}
  <div class="modal-foot">
    <button class="primary" disabled={draft.busy} onclick={confirm}>{draft.busy ? "保存中…" : "保存快照"}</button>
    <button class="cancel" onclick={onClose}>取消</button>
  </div>
</Modal>
