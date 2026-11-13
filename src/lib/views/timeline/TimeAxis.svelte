<script lang="ts">
  // ux-spec 3.4 TimeAxis: labelled ticks at least 80 px apart; drag to zoom into a range.
  import { axisLabel, pickStep, tickTimes, toT, toX, type Scale } from './timeScale';

  let {
    scale,
    width,
    mode,
    origin,
    onrange,
  }: { scale: Scale; width: number; mode: 'elapsed' | 'clock'; origin: number; onrange: (a: number, b: number) => void } = $props();

  const step = $derived(pickStep(scale.msPerPx));
  const ticks = $derived(tickTimes(scale.t0, toT(scale, width), step, origin));
  let drag = $state<{ x0: number; x1: number } | null>(null);
  let el: SVGSVGElement | undefined = $state();

  function x(e: PointerEvent) {
    const r = el!.getBoundingClientRect();
    return Math.max(0, Math.min(width, e.clientX - r.left));
  }
  function down(e: PointerEvent) {
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    const p = x(e);
    drag = { x0: p, x1: p };
  }
  function move(e: PointerEvent) {
    if (drag) drag = { ...drag, x1: x(e) };
  }
  function up() {
    if (drag && Math.abs(drag.x1 - drag.x0) > 6) {
      const a = Math.min(drag.x0, drag.x1);
      const b = Math.max(drag.x0, drag.x1);
      onrange(toT(scale, a), toT(scale, b));
    }
    drag = null;
  }
</script>

<svg class="axis" bind:this={el} {width} height="28" role="slider" aria-label="Time axis. Drag to zoom into a range." aria-valuenow={Math.round(scale.t0)} tabindex="-1" onpointerdown={down} onpointermove={move} onpointerup={up} onpointercancel={() => (drag = null)}>
  {#each ticks as t (t)}
    {@const px = Math.round(toX(scale, t)) + 0.5}
    <line class="tick" x1={px} x2={px} y1="20" y2="28" />
    <text class="label num" x={px + 4} y="16">{axisLabel(t, origin, mode, step)}</text>
  {/each}
  {#if drag}
    <rect class="brush" x={Math.min(drag.x0, drag.x1)} y="0" width={Math.abs(drag.x1 - drag.x0)} height="28" />
  {/if}
</svg>

<style>
  .axis { display: block; cursor: col-resize; user-select: none; }
  .tick { stroke: var(--border-strong); stroke-width: 1; }
  .label { fill: var(--text-muted); font-size: var(--text-xs); font-family: var(--font-ui); }
  .brush { fill: var(--accent-soft); stroke: var(--accent); stroke-width: 1; opacity: 0.8; }
</style>
