<script lang="ts">
  import Modal from "../Modal.svelte";
  import { api, type ProfileInfo } from "$lib/api";
  import { monitor, loadProfiles } from "$lib/monitor.svelte";
  import { toast } from "$lib/ui.svelte";

  let { profile, onClose }: { profile: ProfileInfo; onClose: () => void } = $props();

  let busy = $state(false);

  async function confirm() {
    busy = true;
    try {
      await api.removeProfile(profile.name);
      // 清理该 email 的缓存状态，避免删除后留下孤儿键
      delete monitor.accounts[profile.email];
      toast(`已删除档案 “${profile.name}”`);
      onClose();
      await loadProfiles();
    } catch (e) {
      toast(String(e), "err");
      busy = false;
    }
  }
</script>

<Modal title="删除档案" {onClose}>
  <p class="confirm">确定删除档案 <b>{profile.name}</b>？<br /><span class="cmuted">{profile.email}</span></p>
  <p class="hint">仅删除本地快照与其监控凭证，不影响当前已登录的账号。</p>
  <div class="modal-foot">
    <button class="danger" disabled={busy} onclick={confirm}>{busy ? "删除中…" : "删除"}</button>
    <button class="cancel" onclick={onClose}>取消</button>
  </div>
</Modal>
