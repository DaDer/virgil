<script lang="ts">
  // Bar sparkline. Bars scale to the series' own maximum; an empty series is a flat 1 px line.
  let {
    values,
    width = 240,
    height = 32,
    highlightLast = 0,
    label = 'Activity',
  }: { values: number[]; width?: number; height?: number; highlightLast?: number; label?: string } = $props();

  const bars = $derived.by(() => {
    const n = values.length;
    const max = Math.max(0, ...values);
    if (!n || max === 0) return null;
    const step = width / n;
    const w = Math.max(1, step - Math.min(2, step * 0.3));
    return values.map((v, i) => {
      const h = v > 0 ? Math.max(2, (v / max) * (height - 1)) : 1;
      return { x: i * step, y: height - h, w, h, hot: i >= n - highlightLast && v > 0, zero: v === 0 };
    });
  });
</script>

<svg class="spark" viewBox="0 0 {width} {height}" width="100%" {height} preserveAspectRatio="none" role="img" aria-label={label}>
  {#if bars}
    {#each bars as b, i (i)}
      <rect x={b.x} y={b.y} width={b.w} height={b.h} rx="1" class:hot={b.hot} class:zero={b.zero} />
    {/each}
  {:else}
    <rect x="0" y={height - 1} {width} height="1" class="zero" />
  {/if}
</svg>

<style>
  .spark { display: block; overflow: visible; }
  rect { fill: var(--text-muted); opacity: 0.7; }
  rect.zero { opacity: 0.3; }
  rect.hot { fill: var(--state-running); opacity: 1; }
</style>
