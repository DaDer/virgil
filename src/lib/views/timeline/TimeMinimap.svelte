<script lang="ts">
  // ux-spec 3.4 TimeMinimap: the whole range, a tool-call histogram, run spans, and a brush for
  // the visible window (drag to move, drag an edge to resize, click to centre).
  let {
    range,
    window: win,
    density,
    runs,
    width,
    onwindow,
  }: {
    range: [number, number];
    window: [number, number];
    density: number[];
    runs: { start: number; end: number }[];
    width: number;
    onwindow: (a: number, b: number) => void;
  } = $props();

  const H = 40;
  const span = $derived(Math.max(1, range[1] - range[0]));
  const px = (t: number) => ((t - range[0]) / span) * width;
  const tAt = (x: number) => range[0] + (x / Math.max(1, width)) * span;
  const max = $derived(Math.max(1, ...density));
  const bw = $derived(width / Math.max(1, density.length));
  const bx = $derived([Math.max(0, px(win[0])), Math.min(width, px(win[1]))] as const);

  let el: SVGSVGElement | undefined = $state();
  let drag: { mode: 'move' | 'left' | 'right'; x: number; a: number; b: number } | null = null;

  function local(e: PointerEvent) {
    return e.clientX - el!.getBoundingClientRect().left;
  }
  function down(e: PointerEvent) {
    const x = local(e);
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    const [l, r] = bx;
    const mode = Math.abs(x - l) < 6 ? 'left' : Math.abs(x - r) < 6 ? 'right' : x > l && x < r ? 'move' : null;
    if (!mode) {
      const half = (win[1] - win[0]) / 2;
      const c = tAt(x);
      onwindow(c - half, c + half);
      drag = { mode: 'move', x, a: c - half, b: c + half };
      return;
    }
    drag = { mode, x, a: win[0], b: win[1] };
  }
  function move(e: PointerEvent) {
    if (!drag) return;
    const dt = ((local(e) - drag.x) / Math.max(1, width)) * span;
    if (drag.mode === 'move') onwindow(drag.a + dt, drag.b + dt);
    else if (drag.mode === 'left') onwindow(Math.min(drag.a + dt, drag.b - 1000), drag.b);
    else onwindow(drag.a, Math.max(drag.b + dt, drag.a + 1000));
  }
</script>

<svg class="mini" bind:this={el} {width} height={H} role="slider" aria-label="Visible time window" aria-valuenow={Math.round(win[0])} tabindex="-1" onpointerdown={down} onpointermove={move} onpointerup={() => (drag = null)} onpointercancel={() => (drag = null)}>
  {#each density as v, i (i)}
    {#if v > 0}
      {@const h = Math.max(1, (v / max) * (H - 10))}
      <rect class="bar" x={i * bw} y={H - 6 - h} width={Math.max(1, bw - 1)} height={h} />
    {/if}
  {/each}
  {#each runs as r, i (i)}
    <rect class="run" x={px(r.start)} y={H - 3} width={Math.max(2, px(r.end) - px(r.start))} height="2" />
  {/each}
  <rect class="brush" x={bx[0]} y="1" width={Math.max(4, bx[1] - bx[0])} height={H - 2} rx="2" />
  <rect class="edge" x={bx[0] - 1} y="8" width="3" height={H - 16} rx="1" />
  <rect class="edge" x={bx[1] - 2} y="8" width="3" height={H - 16} rx="1" />
</svg>

<style>
  .mini { display: block; cursor: grab; user-select: none; }
  .bar { fill: var(--text-muted); opacity: 0.5; }
  .run { fill: var(--accent); }
  .brush { fill: var(--accent-soft); fill-opacity: 0.45; stroke: var(--accent); stroke-width: 1; }
  .edge { fill: var(--accent); cursor: ew-resize; }
</style>
