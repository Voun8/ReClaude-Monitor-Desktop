<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { api, type MonitorSnapshot } from "$lib/api";
  import { fmtUsdCompact } from "$lib/format";
  import { dragOrClick } from "$lib/actions/dragOrClick";
  import { fetchFloatSnapshot, getFloatIntervalMs, setupFloatWindow } from "$lib/float";

  let snapshot = $state<MonitorSnapshot | null>(null);
  let status = $state<"loading" | "ok" | "nocred" | "error">("loading");
  let timer: ReturnType<typeof setInterval>;
  let unlistenClose: (() => void) | undefined;

  // status 归一化、badCredentials 判读与 snapshot 回写刻意留在本组件（有状态/跨进程，
  // 与主窗口各自维护、不共享）；下沉到 $lib/float 的只有无副作用的纯拉取链。
  async function load() {
    try {
      const res = await fetchFloatSnapshot();
      if (res.kind === "nocred") {
        status = "nocred";
        return;
      }
      const snap = res.snapshot;
      if (snap.badCredentials) {
        status = "error";
        return;
      }
      snapshot = snap;
      status = snap.quota || snap.metrics ? "ok" : "error";
    } catch {
      status = snapshot ? "ok" : "error";
    }
  }

  const quota = $derived(snapshot?.quota ?? null);
  const metrics = $derived(snapshot?.metrics ?? null);

  // 水位高度 = 用量占比（直接用后端 ratio，仅钳制）
  const ratio = $derived(quota ? Math.max(0, Math.min(1, quota.ratio)) : 0);
  // 水面距顶部（水位越高，top 越小）
  const waterTop = $derived(`${Math.round((1 - ratio) * 100)}%`);

  // 字体显示错误率；颜色按服务健康
  const errText = $derived(
    metrics
      ? metrics.errorRatePct.toFixed(metrics.errorRatePct >= 10 ? 0 : 1)
      : "—",
  );
  const level = $derived(metrics?.stateLevel ?? "ok");

  // 拖拽 vs 点击判定下沉到 use:dragOrClick；点击（未拖拽）回到主面板。
  function onKey(e: KeyboardEvent) {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      api.restoreFromFloat();
    }
  }

  onMount(async () => {
    unlistenClose = await setupFloatWindow();
    const intervalMs = await getFloatIntervalMs();
    load();
    timer = setInterval(load, intervalMs);
  });
  onDestroy(() => {
    clearInterval(timer);
    unlistenClose?.();
  });
</script>

<div class="bubble-container">
  <div
    class="bubble {level}"
    use:dragOrClick={{ onClick: () => api.restoreFromFloat() }}
    onkeydown={onKey}
    role="button"
    tabindex="0"
    title="点击打开主面板"
  >
    <!-- 命中层：铺满整圆的独立合成层，统一接住整球点击。
         透明窗口下静态球面的指针命中不稳定（之前只有带 transform 动画的波浪能点中，
         水位以上没波浪的区域点不动）；本层用 translateZ(0) 提升为合成层，全圆都能响应。 -->
    <div class="hit"></div>
    {#if status === "ok"}
      <div class="bubble-wave" style="--water-level:{waterTop}">
        <div class="wave wave1"></div>
        <div class="wave wave2"></div>
      </div>
      <div class="bubble-content">
        <span class="cap">错误率</span>
        <span class="big">{errText}<span class="pct">%</span></span>
        <span class="usage">
          {quota ? `${fmtUsdCompact(quota.usedUsd)} / ${fmtUsdCompact(quota.totalUsd)}` : "额度未启用"}
        </span>
      </div>
    {:else}
      <div class="bubble-content">
        <span class="hint">
          {status === "loading"
            ? "加载中…"
            : status === "nocred"
              ? "未配置凭证"
              : "获取失败"}
        </span>
      </div>
    {/if}
  </div>
</div>

<style>
  /* 容器裁切成圆：圆外（含阴影）一律裁掉，四角干净透明 */
  /* pointer-events:none → 透明四角不拦截鼠标，点击穿透到背后窗口 */
  .bubble-container {
    width: 100vw;
    height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    overflow: hidden;
    pointer-events: none;
  }

  .bubble {
    --c: 74, 222, 128; /* 默认绿色（ok） */
    width: 94%;
    height: 94%;
    pointer-events: auto;
    border-radius: 50%;
    background: linear-gradient(
      145deg,
      rgba(30, 35, 45, 0.98),
      rgba(15, 18, 25, 0.98)
    );
    border: 2px solid rgba(255, 255, 255, 0.1);
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    outline: none;
    user-select: none;
    -webkit-user-select: none;
    -webkit-tap-highlight-color: transparent;
    position: relative;
    overflow: hidden;
    box-shadow:
      0 4px 15px rgba(0, 0, 0, 0.3),
      inset 0 1px 1px rgba(255, 255, 255, 0.05);
    transition:
      transform 0.2s ease,
      box-shadow 0.2s ease,
      border-color 0.2s ease;
  }
  .bubble:hover {
    transform: scale(1.03);
  }
  .bubble:focus,
  .bubble:focus-visible {
    outline: none;
  }

  .bubble.warn {
    --c: 251, 191, 36;
  }
  .bubble.err {
    --c: 248, 113, 113;
  }
  .bubble {
    border-color: rgba(var(--c), 0.5);
    box-shadow:
      0 0 25px rgba(var(--c), 0.22),
      0 4px 15px rgba(0, 0, 0, 0.3),
      inset 0 1px 1px rgba(255, 255, 255, 0.05);
  }

  /* 命中层：铺满整圆 + translateZ(0) 独立合成层，保证整球都能接住点击（透明窗口下静态面命中不稳） */
  .hit {
    position: absolute;
    inset: 0;
    z-index: 5;
    border-radius: 50%;
    transform: translateZ(0);
    cursor: pointer;
  }

  /* 水波容器：top 即水面位置（用量越高越靠上） */
  .bubble-wave {
    position: absolute;
    top: var(--water-level, 50%);
    left: -10%;
    width: 120%;
    height: 120%;
    transition: top 0.8s cubic-bezier(0.4, 0, 0.2, 1);
  }
  .wave {
    position: absolute;
    width: 200%;
    height: 200%;
    top: 0;
    left: 50%;
    border-radius: 43%;
    transform: translateX(-50%);
    animation: wave 8s infinite linear;
    opacity: 0.85;
  }
  .wave1 {
    background: rgba(var(--c), 0.4);
    animation-duration: 7s;
  }
  .wave2 {
    background: rgba(var(--c), 0.3);
    animation-duration: 11s;
    animation-delay: -3s;
  }
  @keyframes wave {
    0% {
      transform: translateX(-50%) rotate(0deg);
    }
    100% {
      transform: translateX(-50%) rotate(360deg);
    }
  }

  .bubble-content {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1vmin;
    position: relative;
    z-index: 1;
    padding: 4px;
    pointer-events: none;
    text-align: center;
  }
  .cap {
    font-size: 13vmin;
    font-weight: 600;
    letter-spacing: 0.06em;
    color: rgba(255, 255, 255, 0.75);
    text-shadow: 0 1px 4px rgba(0, 0, 0, 0.7);
  }
  .big {
    font-size: 27vmin;
    font-weight: 800;
    line-height: 1;
    color: #fff;
    font-variant-numeric: tabular-nums;
    text-shadow: 0 2px 8px rgba(0, 0, 0, 0.8);
  }
  .big .pct {
    font-size: 13vmin;
    font-weight: 700;
    margin-left: 0.5vmin;
  }
  .usage {
    margin-top: 1.4vmin;
    font-size: 10.5vmin;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.9);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    text-shadow: 0 1px 5px rgba(0, 0, 0, 0.8);
  }
  .hint {
    font-size: 10vmin;
    color: rgba(255, 255, 255, 0.7);
    text-shadow: 0 1px 4px rgba(0, 0, 0, 0.7);
  }
</style>
