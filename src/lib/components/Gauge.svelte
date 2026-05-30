<script lang="ts">
  let {
    ratio = 0,
    size = 176,
    stroke = 14,
    color = "var(--accent)",
    big = "",
    small = "",
  }: {
    ratio?: number;
    size?: number;
    stroke?: number;
    color?: string;
    big?: string;
    small?: string;
  } = $props();

  const r = $derived((size - stroke) / 2);
  const circ = $derived(2 * Math.PI * r);
  const clamped = $derived(Math.max(0, Math.min(1, ratio)));
  const offset = $derived(circ * (1 - clamped));
  const center = $derived(size / 2);
</script>

<div class="gauge" style="width:{size}px;height:{size}px">
  <svg width={size} height={size} viewBox="0 0 {size} {size}">
    <circle
      cx={center}
      cy={center}
      {r}
      fill="none"
      stroke="var(--track)"
      stroke-width={stroke}
    />
    <circle
      cx={center}
      cy={center}
      {r}
      fill="none"
      stroke={color}
      stroke-width={stroke}
      stroke-linecap="round"
      stroke-dasharray={circ}
      stroke-dashoffset={offset}
      transform="rotate(-90 {center} {center})"
      class="arc"
    />
  </svg>
  <div class="center">
    <div class="big" style="color:{color}">{big}</div>
    {#if small}<div class="small">{small}</div>{/if}
  </div>
</div>

<style>
  .gauge {
    position: relative;
    display: inline-grid;
    place-items: center;
  }
  svg {
    transform: rotate(0deg);
  }
  .arc {
    transition: stroke-dashoffset 0.6s cubic-bezier(0.4, 0, 0.2, 1),
      stroke 0.4s ease;
  }
  .center {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
  }
  .big {
    font-size: 34px;
    font-weight: 800;
    letter-spacing: -0.02em;
    line-height: 1;
  }
  .small {
    font-size: 12px;
    color: var(--muted);
    font-weight: 500;
  }
</style>
