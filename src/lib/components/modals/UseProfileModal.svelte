<script lang="ts">
  import Modal from "../Modal.svelte";
  import type { ProfileInfo } from "$lib/api";
  import { switchTo } from "$lib/monitor.svelte";
  import { settings, setNoApp } from "$lib/settings.svelte";

  let { profile, onClose }: { profile: ProfileInfo; onClose: () => void } = $props();

  function confirm() {
    onClose();
    void switchTo(profile, settings.noApp);
  }
</script>

<Modal title="切换账号" {onClose}>
  <p class="confirm">将切换到 <b>{profile.name}</b><br /><span class="cmuted">{profile.email}</span></p>
  <p class="hint">会停止桌面 App 与 daemon，写入该账号凭证{profile.hasAppSession ? "、恢复 App 会话" : ""}，然后{settings.noApp ? "仅拉起 daemon" : "打开桌面 App"}。</p>
  <label class="cbox">
    <input
      type="checkbox"
      checked={settings.noApp}
      onchange={(e) => setNoApp(e.currentTarget.checked)}
    />
    <span>只换凭证，不自动打开桌面 App</span>
  </label>
  <div class="modal-foot">
    <button class="primary" onclick={confirm}>确认切换</button>
    <button class="cancel" onclick={onClose}>取消</button>
  </div>
</Modal>
