<script lang="ts">
  import { onDestroy } from "svelte";
  import { Chart } from "@antv/g2";
  import type { HeatCell } from "$lib/api";

  let { cells, weeks = 53 }: { cells: HeatCell[]; weeks?: number } = $props();

  function toKey(d: Date): string {
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, "0");
    const day = String(d.getDate()).padStart(2, "0");
    return `${y}-${m}-${day}`;
  }

  function normDate(raw: string): string | null {
    if (/^\d{4}-\d{2}-\d{2}/.test(raw)) return raw.slice(0, 10);
    const n = Number(raw);
    if (raw.trim() !== "" && !Number.isNaN(n)) {
      const ms = n < 1e12 ? n * 1000 : n; // 秒 → 毫秒
      const d = new Date(ms);
      if (!Number.isNaN(d.getTime())) return toKey(d);
    }
    const d = new Date(raw);
    return Number.isNaN(d.getTime()) ? null : toKey(d);
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

  function cssVar(name: string, fallback: string): string {
    const v = getComputedStyle(document.documentElement)
      .getPropertyValue(name)
      .trim();
    return v || fallback;
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
    const dark = !window.matchMedia("(prefers-color-scheme: light)").matches;
    const accent = cssVar("--accent", "#d97757");
    const track = cssVar("--track", "rgba(255,255,255,0.09)");
    const ramp = [
      track,
      rgba(accent, 0.3),
      rgba(accent, 0.52),
      rgba(accent, 0.76),
      accent,
    ];
    const colorOf = (c: number) =>
      c <= 0 ? ramp[0] : ramp[Math.min(4, Math.ceil((c / maxCount) * 4))];

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
            valueFormatter: (v: number) => {
              const n = Math.abs(v);
              if (n >= 1e9) return (v / 1e9).toFixed(2) + "B";
              if (n >= 1e6) return (v / 1e6).toFixed(2) + "M";
              if (n >= 1e3) return (v / 1e3).toFixed(2) + "k";
              return String(Math.round(v));
            },
          },
        ],
      });

    chart.render();
  }

  // 数据变化 → 重建（宽度随周数变化）
  $effect(() => {
    void days;
    void weekCount;
    if (!el) {
      chart?.destroy();
      chart = null;
      return;
    }
    chart?.destroy();
    build();
  });

  onDestroy(() => chart?.destroy());
</script>

<div class="heat">
  <div class="scroll">
    <div class="g2" bind:this={el}></div>
  </div>
  <div class="legend">
    <span>少</span>
    <div class="cell lvl-0"></div>
    <div class="cell lvl-1"></div>
    <div class="cell lvl-2"></div>
    <div class="cell lvl-3"></div>
    <div class="cell lvl-4"></div>
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
  .lvl-0 {
    background: var(--track);
  }
  .lvl-1 {
    background: color-mix(in srgb, var(--accent) 30%, transparent);
  }
  .lvl-2 {
    background: color-mix(in srgb, var(--accent) 52%, transparent);
  }
  .lvl-3 {
    background: color-mix(in srgb, var(--accent) 76%, transparent);
  }
  .lvl-4 {
    background: var(--accent);
  }
</style>
