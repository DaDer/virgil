<script lang="ts">
  // ux-spec 2.7: density strip, phase and run marks, hatched quiet gaps, playhead.
  // Click seeks; drag scrubs (onscrub, throttled by the clock); release seeks exactly (onseek).
  import { formatClock, formatElapsed } from '../format';

  export interface ScrubMarker {
    t: number;
    label: string;
    kind: 'phase' | 'run';
    /** For runs: end of the underline. */
    end?: number;
  }

  let {
    range,
    value,
    density,
    markers = [],
    gaps = [],
    onseek,
    onscrub,
    hoverText,
  }: {
    range: [number, number];
    value: number;
    density: number[];
    markers?: ScrubMarker[];
    gaps?: [number, number][];
    onseek: (t: number) => void;
    onscrub?: (t: number) => void;
    hoverText?: (t: number) => string;
  } = $props();

  let el: HTMLDivElement | undefined = $state();
  let hoverX = $state<number | null>(null);
  let dragging = false;

  const span = $derived(Math.max(1, range[1] - range[0]));
  const pct = (t: number) => ((Math.min(range[1], Math.max(range[0], t)) - range[0]) / span) * 100;
  const max = $derived(Math.max(1, ...density));

  function at(e: PointerEvent): number {
    const r = el!.getBoundingClientRect();
    return range[0] + Math.min(1, Math.max(0, (e.clientX - r.left) / r.width)) * span;
  }

  function down(e: PointerEvent) {
    if (e.button !== 0) return;
    dragging = true;
    el!.setPointerCapture(e.pointerId);
    onscrub ? onscrub(at(e)) : onseek(at(e));
  }
  function move(e: PointerEvent) {
    const r = el!.getBoundingClientRect();
    hoverX = e.clientX - r.left;
    if (dragging) onscrub?.(at(e));
  }
  function up(e: PointerEvent) {
    if (!dragging) return;
    dragging = false;
    onseek(at(e));
  }
  function key(e: KeyboardEvent) {
    const step = e.shiftKey ? 60_000 : 5_000;
    if (e.key === 'ArrowLeft') onseek(value - step);
    else if (e.key === 'ArrowRight') onseek(value + step);
    else if (e.key === 'Home') onseek(range[0]);
    else if (e.key === 'End') onseek(range[1]);
    else return;
    e.preventDefault();
    e.stopPropagation();
  }

  const hoverT = $derived(hoverX != null && el ? range[0] + (hoverX / el.clientWidth) * span : null);
</script>

<div
  class="scrub"
  bind:this={el}
  role="slider"
  tabindex="0"
  aria-label="Playhead"
  aria-valuemin={range[0]}
  aria-valuemax={range[1]}
  aria-valuenow={Math.round(value)}
  aria-valuetext="{formatElapsed(value - range[0])} of {formatElapsed(span)}"
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointerleave={() => (hoverX = null)}
  onkeydown={key}
>
  <svg class="density" viewBox="0 0 {Math.max(1, density.length)} 100" preserveAspectRatio="none" aria-hidden="true">
    {#each density as d, i (i)}
      {#if d > 0}<rect x={i} y={100 - (d / max) * 100} width="0.9" height={(d / max) * 100} />{/if}
    {/each}
  </svg>
  {#each gaps as g, i (i)}
    <span class="gap" style="left: {pct(g[0])}%; width: {pct(g[1]) - pct(g[0])}%" aria-hidden="true"></span>
  {/each}
  {#each markers as m, i (i)}
    {#if m.kind === 'phase'}
      <span class="mark" style="left: {pct(m.t)}%" title={m.label}></span>
    {:else}
      <span class="run" style="left: {pct(m.t)}%; width: {pct(m.end ?? range[1]) - pct(m.t)}%" title={m.label}></span>
    {/if}
  {/each}
  <span class="played" style="width: {pct(value)}%" aria-hidden="true"></span>
  <span class="head" style="left: {pct(value)}%" aria-hidden="true"><span class="knob"></span></span>
  {#if hoverT != null && hoverX != null}
    <span class="tip t-small num" style="left: {hoverX}px">
      {formatElapsed(hoverT - range[0])} · {formatClock(hoverT)}{hoverText ? ` · ${hoverText(hoverT)}` : ''}
    </span>
  {/if}
</div>

<style>
  .scrub {
    position: relative;
    flex: 1;
    min-width: 120px;
    height: 28px;
    border-radius: var(--radius-sm);
    background: var(--surface-sunken);
    cursor: pointer;
    touch-action: none;
  }
  .density { position: absolute; inset: 2px 0 4px; width: 100%; height: calc(100% - 6px); }
  .density rect { fill: var(--text-muted); opacity: 0.5; }
  .gap {
    position: absolute;
    top: 0;
    bottom: 0;
    background: repeating-linear-gradient(135deg, var(--border-default) 0 1px, transparent 1px 5px);
    opacity: 0.8;
    pointer-events: none;
  }
  .mark { position: absolute; top: 0; bottom: 0; width: 1px; background: var(--border-strong); }
  .run { position: absolute; bottom: 0; height: 2px; background: var(--accent); }
  .played { position: absolute; left: 0; top: 0; bottom: 0; background: var(--accent-soft); opacity: 0.5; pointer-events: none; }
  .head { position: absolute; top: -3px; bottom: -3px; width: 2px; margin-left: -1px; background: var(--accent); pointer-events: none; }
  .knob { position: absolute; top: -4px; left: -4px; width: 10px; height: 10px; border-radius: 50%; background: var(--accent); box-shadow: 0 0 0 2px var(--surface-panel); }
  .tip {
    position: absolute;
    bottom: calc(100% + 8px);
    transform: translateX(-50%);
    padding: 2px var(--space-2);
    border-radius: var(--radius-sm);
    background: var(--surface-raised);
    border: var(--border-width) solid var(--border-default);
    box-shadow: var(--shadow-3);
    color: var(--text-primary);
    white-space: nowrap;
    pointer-events: none;
    z-index: var(--z-popover);
  }
</style>
