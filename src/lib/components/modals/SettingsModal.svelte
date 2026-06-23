<script lang="ts">
  import Modal from "../Modal.svelte";
  import {
    settings,
    saveSettings,
    REFRESH_SEC_MIN,
    REFRESH_SEC_MAX,
    FLOAT_SIZE_MIN,
    FLOAT_SIZE_MAX,
  } from "$lib/settings.svelte";
  import { toast } from "$lib/ui.svelte";

  let {
    onClose,
    onSaved,
  }: { onClose: () => void; onSaved: (apiChanged: boolean) => void } = $props();

  // 草稿字段与正式设置同名，确认时整体回写，不做字段改名映射
  const draft = $state({
    refreshSec: settings.refreshSec,
    floatMode: settings.floatMode,
    floatSize: settings.floatSize,
    apiBase: settings.apiBase,
    apiKey: settings.apiKey,
    closeAction: settings.closeAction,
    silentStart: settings.silentStart,
    autostart: settings.autostart,
  });

  const PRESETS = [10, 30, 60, 300];

  function confirm() {
    const r = saveSettings(draft);
    if (!r.ok) {
      toast(r.error, "err");
      return;
    }
    onClose();
    toast("设置已保存");
    onSaved(r.apiChanged);
  }
</script>

<Modal title="设置" {onClose}>
  <div class="field">
    <label for="s-sec">自动刷新间隔（秒）</label>
    <input id="s-sec" type="number" min={REFRESH_SEC_MIN} max={REFRESH_SEC_MAX} step="5" bind:value={draft.refreshSec} />
  </div>
  <div class="presets">
    {#each PRESETS as s (s)}
      <button class="preset" class:on={draft.refreshSec === s} onclick={() => (draft.refreshSec = s)}>
        {s < 60 ? `${s}s` : `${s / 60}m`}
      </button>
    {/each}
  </div>
  <p class="hint">额度与服务指标的自动刷新频率（{REFRESH_SEC_MIN}–{REFRESH_SEC_MAX} 秒）。倒计时与跟随账号不受影响。</p>

  <div class="field set-sep">
    <label for="s-api">中转地址</label>
    <input id="s-api" type="text" placeholder="如 https://proxy.mortysky.top" bind:value={draft.apiBase} />
  </div>
  <p class="hint">中转服务地址；留空使用默认中转。上游 failover 由中转负责。</p>

  <div class="field">
    <label for="s-key">访问令牌（API Key）</label>
    <input id="s-key" type="password" placeholder="中转为本账号签发的 Key" bind:value={draft.apiKey} />
  </div>
  <p class="hint">中转启用鉴权时必填，请求会带上 x-api-key 头。</p>

  <div class="field set-sep">
    <div class="seg-title">最小化方式</div>
    <div class="presets">
      <button class="preset" class:on={draft.floatMode === "ball"} onclick={() => (draft.floatMode = "ball")}>悬浮球</button>
      <button class="preset" class:on={draft.floatMode === "tray"} onclick={() => (draft.floatMode = "tray")}>菜单栏圆环</button>
    </div>
  </div>
  {#if draft.floatMode === "ball"}
    <div class="field">
      <label for="s-size">悬浮球大小（px，正方形）</label>
      <input id="s-size" type="number" min={FLOAT_SIZE_MIN} max={FLOAT_SIZE_MAX} step="10" bind:value={draft.floatSize} />
    </div>
  {:else}
    <p class="hint">菜单栏圆环显示「可用余额百分比」，点击图标打开主面板。</p>
  {/if}

  <div class="field set-sep">
    <div class="seg-title">关闭窗口时</div>
    <div class="presets">
      <button class="preset" class:on={draft.closeAction === "quit"} onclick={() => (draft.closeAction = "quit")}>退出程序</button>
      <button class="preset" class:on={draft.closeAction === "background"} onclick={() => (draft.closeAction = "background")}>后台运行</button>
    </div>
  </div>

  <div class="field set-sep">
    <div class="seg-title">开机自启动</div>
    <div class="presets">
      <button class="preset" class:on={draft.autostart} onclick={() => (draft.autostart = true)}>开启</button>
      <button class="preset" class:on={!draft.autostart} onclick={() => (draft.autostart = false)}>关闭</button>
    </div>
  </div>

  <div class="field">
    <div class="seg-title">静默启动</div>
    <div class="presets">
      <button class="preset" class:on={draft.silentStart} onclick={() => (draft.silentStart = true)}>开启</button>
      <button class="preset" class:on={!draft.silentStart} onclick={() => (draft.silentStart = false)}>关闭</button>
    </div>
  </div>
  <p class="hint">
    开机自启动登记到系统登录项。静默启动开启时，启动直接进{draft.floatMode === "tray" ? "菜单栏圆环" : "悬浮球"}、不弹主窗口（首次启动除外）；关闭则启动显示主窗口。
  </p>

  <div class="modal-foot">
    <button class="primary" onclick={confirm}>保存</button>
    <button class="cancel" onclick={onClose}>取消</button>
  </div>
</Modal>

<style>
  .presets {
    display: flex;
    gap: 7px;
    margin: 10px 0 2px;
  }
  .preset {
    flex: 1;
    padding: 7px 0;
    border-radius: 9px;
    border: 1px solid var(--border-strong);
    background: var(--surface-2);
    color: var(--muted);
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
    transition:
      border-color 0.15s ease,
      color 0.15s ease,
      background 0.15s ease;
  }
  .preset:hover {
    border-color: var(--accent);
    color: var(--fg);
  }
  .preset.on {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }
  .set-sep {
    margin-top: 14px;
    padding-top: 14px;
    border-top: 1px solid var(--border);
  }
  .seg-title {
    font-size: 12px;
    color: var(--muted);
    margin-bottom: 7px;
  }
</style>
