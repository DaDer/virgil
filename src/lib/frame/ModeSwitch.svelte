<script lang="ts">
  // ux-spec 2.6: Live / Replay. In replay the segment shows the recording name.
  import Icon from '../components/Icon.svelte';

  let {
    mode,
    recordingName,
    onlive,
    onreplay,
  }: { mode: 'live' | 'replay'; recordingName?: string; onlive: () => void; onreplay: () => void } = $props();
</script>

<div class="switch" role="group" aria-label="Data source">
  <button type="button" class:on={mode === 'live'} aria-pressed={mode === 'live'} onclick={onlive} title="Live (L)">
    <Icon name="live" size={14} /> Live
  </button>
  <button type="button" class:on={mode === 'replay'} class="replay" aria-pressed={mode === 'replay'} onclick={onreplay} title={mode === 'replay' ? 'Open another recording (R)' : 'Replay a recording (R)'}>
    <Icon name="replay" size={14} />
    <span class="ellipsis">{mode === 'replay' && recordingName ? recordingName : 'Replay'}</span>
  </button>
</div>

<style>
  .switch {
    display: inline-flex;
    padding: 2px;
    gap: 2px;
    border-radius: var(--radius-md);
    background: var(--surface-sunken);
    border: var(--border-width) solid var(--border-subtle);
  }
  button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 24px;
    padding: 0 var(--space-3);
    border-radius: var(--radius-sm);
    font-size: var(--text-sm);
    font-weight: var(--weight-medium);
    color: var(--text-secondary);
    max-width: 200px;
    min-width: 0;
  }
  button:hover { color: var(--text-primary); }
  .on { background: var(--surface-card); color: var(--text-primary); box-shadow: var(--shadow-1), inset 0 0 0 var(--border-width) var(--border-default); }
  .replay.on { color: var(--state-queued-text); box-shadow: var(--shadow-1), inset 0 0 0 var(--border-width) var(--state-queued); }
</style>
