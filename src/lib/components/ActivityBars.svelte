<script lang="ts">
  type Point = { date: string; value: number; ts: number };
  let { points, mode = "day" }: { points: Point[]; mode?: "hour" | "day" } =
    $props();

  const max = $derived(Math.max(1, ...points.map((p) => p.value)));

  function shortLabel(p: Point): string {
    const d = new Date(p.ts);
    if (Number.isNaN(d.getTime())) return p.date;
    const mm = String(d.getMonth() + 1).padStart(2, "0");
    const dd = String(d.getDate()).padStart(2, "0");
    const hh = String(d.getHours()).padStart(2, "0");
    return mode === "hour" ? `${hh}:00` : `${mm}-${dd}`;
  }

  const axis = $derived.by(() => {
    if (points.length === 0) return [];
    const idxs = [0, Math.floor((points.length - 1) / 2), points.length - 1];
    return [...new Set(idxs)].map((i) => shortLabel(points[i]));
  });
</script>

{#if points.length === 0}
  <div class="empty">该时间段暂无活动数据</div>
{:else}
  <div class="bars" style="--n:{points.length}">
    {#each points as p (p.date)}
      <div class="slot" title="{shortLabel(p)} · {p.value}">
        <div
          class="bar"
          style="height:{p.value <= 0 ? 2 : Math.max(4, (p.value / max) * 100)}%;opacity:{p.value <= 0 ? 0.25 : 1}"
        ></div>
      </div>
    {/each}
  </div>
  <div class="axis">
    {#each axis as a}<span>{a}</span>{/each}
  </div>
{/if}

<style>
  .empty {
    color: var(--faint);
    font-size: 12.5px;
    text-align: center;
    padding: 24px 0;
  }
  .bars {
    display: flex;
    align-items: flex-end;
    gap: 2px;
    height: 120px;
    padding-top: 4px;
  }
  .slot {
    flex: 1 1 0;
    min-width: 0;
    height: 100%;
    display: flex;
    align-items: flex-end;
    justify-content: center;
  }
  .bar {
    width: 100%;
    max-width: 22px;
    border-radius: 4px 4px 2px 2px;
    background: linear-gradient(
      180deg,
      var(--accent),
      color-mix(in srgb, var(--accent) 55%, transparent)
    );
    transition: height 0.35s ease;
  }
  .slot:hover .bar {
    filter: brightness(1.12);
  }
  .axis {
    display: flex;
    justify-content: space-between;
    margin-top: 7px;
    font-size: 11px;
    color: var(--faint);
  }
</style>
