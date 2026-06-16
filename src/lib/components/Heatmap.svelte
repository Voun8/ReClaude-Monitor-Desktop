<script lang="ts">
  import { onDestroy } from "svelte";
  import { Chart } from "@antv/g2";
  import type { HeatCell } from "$lib/api";
  import { flexDate, fmtTokens } from "$lib/format";
  import { cssVarReader, isDarkTheme } from "$lib/theme";

  let { cells, weeks = 53 }: { cells: HeatCell[]; weeks?: number } = $props();

  function toKey(d: Date): string {
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, "0");
    const day = String(d.getDate()).padStart(2, "0");
    return `${y}-${m}-${day}`;
  }

  function normDate(raw: string): string | null {
    // ISO 串直接截取日期段，避免 date-only 被 Date 按 UTC 解析造成跨时区偏移
    if (/^\d{4}-\d{2}-\d{2}/.test(raw)) return raw.slice(0, 10);
    const d = flexDate(raw);
    return d ? toKey(d) : null;
  }

  type Day = { week: number; weekday: number; date: string; count: number };

  const days = $derived.by<Day[]>(() => {
    const map = new Map<string, number>();
    for (const c of cells) {
      const k = normDate(c.date);
      if (k) map.set(k, (map.get(k) || 0) + (c.count || 0));
    }
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    const start = new Date(today);
    start.setDate(start.getDate() - (weeks * 7 - 1));
    start.setDate(start.getDate() - start.getDay()); // 对齐到周日

    const out: Day[] = [];
    const cur = new Date(start);
    let wk = 0;
    while (cur <= today) {
      const wd = cur.getDay();
      const k = toKey(cur);
      out.push({ week: wk, weekday: wd, date: k, count: map.get(k) || 0 });
      if (wd === 6) wk++;
      cur.setDate(cur.getDate() + 1);
    }
    return out;
  });

  const maxCount = $derived(Math.max(1, ...days.map((d) => d.count)));
  const weekCount = $derived((days.at(-1)?.week ?? 0) + 1);

  let el = $state<HTMLDivElement | null>(null);
  let chart: Chart | null = null;

  // 5 级色阶的透明度阶梯——JS ramp 与图例共用这一份定义
  const HEAT_ALPHAS = [0.3, 0.52, 0.76];

  // ramp 由 build() 按主题设定;colorOf 读顶层 ramp + 当前 maxCount($derived),
  // 使增量 changeData 后颜色按最新 maxCount 计算,而非 build 时闭包里的旧值。
  let ramp: string[] = [];
  function colorOf(c: number): string {
    return c <= 0 ? ramp[0] : ramp[Math.min(4, Math.ceil((c / maxCount) * 4))];
  }

  function rgba(hex: string, a: number): string {
    const h = hex.replace("#", "");
    if (!/^[0-9a-fA-F]{6}$/.test(h)) return hex;
    const r = parseInt(h.slice(0, 2), 16);
    const g = parseInt(h.slice(2, 4), 16);
    const b = parseInt(h.slice(4, 6), 16);
    return `rgba(${r}, ${g}, ${b}, ${a})`;
  }

  function build() {
    if (!el) return;
    const dark = isDarkTheme();
    const v = cssVarReader();
    const accent = v("--accent", "#d97757");
    const track = v("--track", "rgba(255,255,255,0.09)");
    ramp = [track, ...HEAT_ALPHAS.map((a) => rgba(accent, a)), accent];

    chart = new Chart({
      container: el,
      autoFit: false,
      width: weekCount * 15 + 8,
      height: 7 * 15 + 6,
      theme: dark ? "classicDark" : "classic",
      paddingLeft: 4,
      paddingRight: 4,
      paddingTop: 3,
      paddingBottom: 3,
    });

    chart
      .cell()
      .data(days)
      .encode("x", "week")
      .encode("y", "weekday")
      .encode("color", (d: Day) => colorOf(d.count))
      .scale("color", { type: "identity" })
      .scale("x", { paddingInner: 0.18 })
      .scale("y", { paddingInner: 0.18, domain: [0, 1, 2, 3, 4, 5, 6] })
      .style("radius", 2)
      .axis(false)
      .legend(false)
      .tooltip({
        title: (d: Day) => d.date,
        items: [
          {
            field: "count",
            name: "活动",
            valueFormatter: (v: number) => fmtTokens(v, 2),
          },
        ],
      });

    chart.render();
  }

  let lastDark: boolean | null = null;
  let lastWidth = 0;

  // 数据变化 → 增量 changeData，避免销毁重建整年热力图（~371 cell）；
  // 仅首次挂载或主题切换时重建（需重读主题色/ramp）。
  $effect(() => {
    void days;
    void weekCount;
    if (!el) {
      chart?.destroy();
      chart = null;
      lastDark = null;
      return;
    }
    const dark = isDarkTheme();
    if (!chart || dark !== lastDark) {
      chart?.destroy();
      build();
      lastDark = dark;
      lastWidth = weekCount * 15 + 8;
    } else {
      const w = weekCount * 15 + 8;
      if (w !== lastWidth) {
        chart.changeSize(w, 7 * 15 + 6);
        lastWidth = w;
      }
      chart.changeData(days);
    }
  });

  onDestroy(() => chart?.destroy());
</script>

<div class="heat">
  <div class="scroll">
    <div class="g2" bind:this={el}></div>
  </div>
  <div class="legend">
    <span>少</span>
    <div class="cell" style="background: var(--track)"></div>
    {#each HEAT_ALPHAS as a (a)}
      <div class="cell" style="background: color-mix(in srgb, var(--accent) {a * 100}%, transparent)"></div>
    {/each}
    <div class="cell" style="background: var(--accent)"></div>
    <span>多</span>
  </div>
</div>

<style>
  .scroll {
    overflow-x: auto;
    padding-bottom: 4px;
  }
  .g2 {
    width: max-content;
  }
  .legend {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-top: 9px;
    font-size: 11px;
    color: var(--faint);
  }
  .legend .cell {
    width: 11px;
    height: 11px;
    border-radius: 3px;
  }
</style>
