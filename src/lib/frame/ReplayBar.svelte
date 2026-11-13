<script lang="ts">
  // ux-spec 2.7: transport, time readout, scrubber, speed menu, Skip quiet gaps.
  import Button from '../components/Button.svelte';
  import IconButton from '../components/IconButton.svelte';
  import Menu from '../components/Menu.svelte';
  import MenuItem from '../components/MenuItem.svelte';
  import { formatClock, formatElapsed } from '../format';
  import { actions } from '../stores/actions';
  import { clock, SPEEDS } from '../stores/clock.svelte';
  import { ui } from '../stores/ui.svelte';
  import Scrubber, { type ScrubMarker } from './Scrubber.svelte';

  let {
    density,
    markers = [],
    gaps = [],
    runningAt,
  }: {
    density: number[];
    markers?: ScrubMarker[];
    gaps?: [number, number][];
    runningAt?: (t: number) => number;
  } = $props();

  let speedBtn: HTMLButtonElement | undefined = $state();
  let speedOpen = $state(false);

  const range = $derived(clock.range);
  const t = $derived(clock.viewTime);
  const atEnd = $derived(!clock.playing && t >= range[1] - 1);
</script>

<div class="replay" role="toolbar" aria-label="Replay controls">
  <div class="transport">
    <IconButton icon="skip-start" label="Start (Home)" onclick={() => clock.toStart()} />
    <IconButton icon="step-back" label="Previous event (,)" onclick={() => clock.step(-1)} />
    <Button variant="primary" icon={clock.playing ? 'pause' : atEnd ? 'restart' : 'play'} onclick={() => actions.togglePlay()}>
      {clock.playing ? 'Pause' : atEnd ? 'Replay from start' : 'Play'}
    </Button>
    <IconButton icon="step-forward" label="Next event (.)" onclick={() => clock.step(1)} />
    <IconButton icon="skip-end" label="End (End)" onclick={() => clock.toEnd()} />
  </div>
  <div class="time num">
    <span class="elapsed">{formatElapsed(t - range[0])}</span>
    <span class="muted">/ {formatElapsed(range[1] - range[0])}</span>
    <span class="wall t-small muted">{formatClock(t)}</span>
  </div>
  <Scrubber
    {range}
    value={t}
    {density}
    {markers}
    {gaps}
    onseek={(x) => clock.endScrub(x)}
    onscrub={(x) => clock.scrub(x)}
    hoverText={runningAt ? (x) => `${runningAt(x)} agents running` : undefined}
  />
  <button type="button" class="speed num" bind:this={speedBtn} aria-haspopup="menu" aria-expanded={speedOpen} onclick={() => (speedOpen = !speedOpen)} title="Speed ([ and ])">
    {clock.speed}x
  </button>
  <Menu anchor={speedBtn} open={speedOpen} onclose={() => (speedOpen = false)} label="Speed" placement="top-end">
    {#each SPEEDS as s (s)}
      <MenuItem checked={clock.speed === s} onselect={() => { speedOpen = false; clock.setSpeed(s); }}>{s}x</MenuItem>
    {/each}
  </Menu>
  <label class="gaps t-small" title={clock.canSkipGaps ? 'Play quiet stretches over 30 s in 1 s' : 'This data source cannot skip quiet gaps yet'}>
    <input type="checkbox" checked={clock.skipGaps} disabled={!clock.canSkipGaps} onchange={(e) => clock.setSkipGaps(e.currentTarget.checked)} />
    Skip quiet gaps
  </label>
  {#if ui.replayHint && !clock.playing}
    <span class="hint t-small">Press <kbd>Space</kbd> to play</span>
  {/if}
</div>

<style>
  .replay {
    grid-area: replay;
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--space-4);
    height: var(--size-replaybar);
    padding: 0 var(--space-4);
    background: var(--surface-panel);
    border-top: var(--border-width) solid var(--border-subtle);
    min-width: 0;
    z-index: var(--z-panel);
  }
  .transport { display: flex; align-items: center; gap: var(--space-1); }
  .transport :global(.btn) { min-width: 104px; }
  .time { display: flex; align-items: baseline; gap: var(--space-1); white-space: nowrap; }
  .elapsed { font-size: var(--text-lg); color: var(--text-primary); }
  .wall { margin-left: var(--space-2); }
  .speed {
    height: var(--size-control);
    padding: 0 var(--space-3);
    border-radius: var(--radius-sm);
    border: var(--border-width) solid var(--border-default);
    background: var(--surface-card);
  }
  .speed:hover { background: var(--surface-hover); }
  .gaps { display: flex; align-items: center; gap: 6px; white-space: nowrap; color: var(--text-secondary); }
  .gaps input { accent-color: var(--accent); }
  .hint {
    position: absolute;
    left: 50%;
    bottom: calc(100% + 8px);
    transform: translateX(-50%);
    padding: var(--space-1) var(--space-3);
    border-radius: var(--radius-pill);
    background: var(--surface-raised);
    border: var(--border-width) solid var(--border-default);
    box-shadow: var(--shadow-3);
    color: var(--text-primary);
    animation: as-fade-in var(--dur-2) var(--ease-out);
  }
  kbd { font-family: var(--font-mono); padding: 0 4px; border-radius: var(--radius-xs); border: var(--border-width) solid var(--border-default); }
  @media (max-width: 1240px) { .wall, .gaps { display: none; } }
</style>
