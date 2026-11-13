<script lang="ts">
  import type { Tone } from '../states';

  let {
    label,
    value,
    sub,
    share,
    tone,
  }: { label: string; value: string | number; sub?: string; share?: number; tone?: Tone } = $props();
</script>

<div class="tile st-{tone ?? 'idle'}" class:toned={!!tone}>
  <span class="t-label label">{label}</span>
  <span class="value num">{value}</span>
  {#if sub}<span class="sub t-small">{sub}</span>{/if}
  {#if share != null}
    <span class="bar" aria-hidden="true"><span style="transform: scaleX({Math.max(0, Math.min(1, share))})"></span></span>
  {/if}
</div>

<style>
  .tile {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 88px;
    padding: var(--pad-2) var(--pad-3);
    border-radius: var(--radius-md);
    background: var(--surface-card);
    border: var(--border-width) solid var(--border-subtle);
  }
  .label { color: var(--text-muted); }
  .toned .label { color: var(--st-text); }
  .value { font-size: var(--text-xl); line-height: var(--leading-xl); color: var(--text-primary); }
  .sub { color: var(--text-secondary); }
  .bar { display: block; height: 3px; margin-top: var(--space-1); border-radius: var(--radius-pill); background: var(--surface-sunken); overflow: hidden; }
  .bar span { display: block; height: 100%; background: var(--accent); transform-origin: left; transition: transform var(--dur-2) var(--ease-out); }
</style>
