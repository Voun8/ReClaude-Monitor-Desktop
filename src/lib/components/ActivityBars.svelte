<script lang="ts">
  import { onDestroy } from "svelte";
  import { Chart } from "@antv/g2";

  type Point = { date: string; value: number; ts: number };
  let { points }: { points: Point[] } = $props();

  let el = $state<HTMLDivElement | null>(null);
  let chart: Chart | null = null;

  function label(p: Point): string {
    const d = new Date(p.ts);
    if (Number.isNaN(d.getTime())) return p.date.slice(0, 10);
    const mm = String(d.getMonth() + 1).padStart(2, "0");
    const dd = String(d.getDate()).padStart(2, "0");
    return `${mm}-${dd}`;
  }

  function cssVar(name: string, fallback: string): string {
    const v = getComputedStyle(document.documentElement)
      .getPropertyValue(name)
      .trim();
    return v || fallback;
  }

  const rows = () =>
    points.map((p) => ({ label: label(p), value: p.value, key: p.date }));

  // 缩写：坐标轴用 0 位小数（141M），tooltip 用 2 位（140.78M）
  function abbr(v: number, d: number): string {
    const n = Math.abs(v);
    if (n >= 1e9) return (v / 1e9).toFixed(d) + "B";
    if (n >= 1e6) return (v / 1e6).toFixed(d) + "M";
    if (n >= 1e3) return (v / 1e3).toFixed(d) + "k";
    return String(Math.round(v));
  }

  function build() {
    if (!el) return;
    const dark = !window.matchMedia("(prefers-color-scheme: light)").matches;
    const accent = cssVar("--accent", "#d97757");
    const faint = cssVar("--faint", "#6b7280");
    const grid = cssVar("--border", "rgba(255,255,255,0.08)");

    chart = new Chart({
      container: el,
      autoFit: true,
      height: 150,
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
      .encode("y", "value")
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
        labelFormatter: (v: number) => abbr(v, 0),
      })
      .tooltip({
        title: (d: { label: string }) => d.label,
        items: [
          { field: "value", name: "活动", valueFormatter: (v: number) => abbr(v, 2) },
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
  <div class="g2" bind:this={el}></div>
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
    height: 150px;
  }
</style>
