<script lang="ts">
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

  type Cell = { key: string; count: number; level: number; label: string };

  const columns = $derived.by<Cell[][]>(() => {
    const map = new Map<string, number>();
    for (const c of cells) {
      const k = normDate(c.date);
      if (k) map.set(k, (map.get(k) || 0) + (c.count || 0));
    }
    const max = Math.max(1, ...map.values());

    const today = new Date();
    today.setHours(0, 0, 0, 0);
    const start = new Date(today);
    start.setDate(start.getDate() - (weeks * 7 - 1));
    start.setDate(start.getDate() - start.getDay()); // 对齐到周日

    const days: Cell[] = [];
    const cur = new Date(start);
    while (cur <= today) {
      const k = toKey(cur);
      const count = map.get(k) || 0;
      const level = count <= 0 ? 0 : Math.min(4, Math.ceil((count / max) * 4));
      days.push({ key: k, count, level, label: `${k} · ${count}` });
      cur.setDate(cur.getDate() + 1);
    }
    const cols: Cell[][] = [];
    for (let i = 0; i < days.length; i += 7) cols.push(days.slice(i, i + 7));
    return cols;
  });
</script>

<div class="heat">
  <div class="scroll">
    <div class="grid">
      {#each columns as col}
        <div class="col">
          {#each col as cell (cell.key)}
            <div class="cell lvl-{cell.level}" title={cell.label}></div>
          {/each}
        </div>
      {/each}
    </div>
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
  .heat {
    --c: 12px;
    --g: 3px;
  }
  .scroll {
    overflow-x: auto;
    padding-bottom: 4px;
  }
  .grid {
    display: flex;
    gap: var(--g);
    width: max-content;
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: var(--g);
  }
  .cell {
    width: var(--c);
    height: var(--c);
    border-radius: 3px;
    background: var(--track);
  }
  .lvl-0 {
    background: var(--track);
  }
  .lvl-1 {
    background: color-mix(in srgb, var(--accent) 28%, transparent);
  }
  .lvl-2 {
    background: color-mix(in srgb, var(--accent) 50%, transparent);
  }
  .lvl-3 {
    background: color-mix(in srgb, var(--accent) 74%, transparent);
  }
  .lvl-4 {
    background: var(--accent);
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
  }
</style>
