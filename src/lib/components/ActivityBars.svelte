<script lang="ts">
  import { onDestroy } from "svelte";
  import { Chart } from "@antv/g2";
  import type { HeatCell } from "$lib/api";
  import { fmtTokens } from "$lib/format";
  import { cssVar, isDarkTheme } from "$lib/theme";

  type Point = HeatCell & { ts: number };
  let { points }: { points: Point[] } = $props();

  const HEIGHT = 150;

  let el = $state<HTMLDivElement | null>(null);
  let chart: Chart | null = null;

  function label(p: Point): string {
    const d = new Date(p.ts);
    if (Number.isNaN(d.getTime())) return p.date.slice(0, 10);
    const mm = String(d.getMonth() + 1).padStart(2, "0");
    const dd = String(d.getDate()).padStart(2, "0");
    return `${mm}-${dd}`;
  }

  const rows = () =>
    points.map((p) => ({ label: label(p), count: p.count, key: p.date }));

  function build() {
    if (!el) return;
    const dark = isDarkTheme();
    const accent = cssVar("--accent", "#d97757");
    const faint = cssVar("--faint", "#6b7280");
    const grid = cssVar("--border", "rgba(255,255,255,0.08)");

    chart = new Chart({
      container: el,
      autoFit: true,
      height: HEIGHT,
      theme: dark ? "classicDark" : "classic",
      paddingTop: 12,
      paddingRight: 8,
      paddingBottom: 22,
      paddingLeft: 44,
    });

    chart
      .interval()
      .data(rows())
      .encode("x", "label")
      .encode("y", "count")
      .scale("y", { nice: true })
      .style("fill", accent)
      .style("radius", 4)
      .style("maxWidth", 22)
      .axis("x", {
        title: false,
        line: false,
        tick: false,
        labelFill: faint,
        labelFontSize: 11,
        labelAutoHide: true,
      })
      .axis("y", {
        title: false,
        labelFill: faint,
        labelFontSize: 11,
        gridStroke: grid,
        tickCount: 4,
        labelFormatter: (v: number) => fmtTokens(v, 0),
      })
      .tooltip({
        title: (d: { label: string }) => d.label,
        items: [
          { field: "count", name: "活动", valueFormatter: (v: number) => fmtTokens(v, 2) },
        ],
      });

    chart.render();
  }

  // 容器挂载 / 数据变化时构建或更新（autoFit 自适应宽度）
  $effect(() => {
    const d = rows(); // 跟踪 points 变化
    if (!el || points.length === 0) {
      chart?.destroy();
      chart = null;
      return;
    }
    if (!chart) build();
    else chart.changeData(d);
  });

  onDestroy(() => chart?.destroy());
</script>

{#if points.length === 0}
  <div class="empty">该时间段暂无活动数据</div>
{:else}
  <div class="g2" style="height:{HEIGHT}px" bind:this={el}></div>
{/if}

<style>
  .empty {
    color: var(--faint);
    font-size: 12.5px;
    text-align: center;
    padding: 24px 0;
  }
  .g2 {
    width: 100%;
  }
</style>
