<script lang="ts">
  import Popover from '../components/Popover.svelte';
  import { formatClock } from '../format';
  import type { Problem, ProblemReason } from '../contract';

  const REASON: Record<ProblemReason, string> = {
    badLine: 'A line is not valid JSON; it was skipped',
    badFile: 'The file could not be parsed',
    truncated: 'The file got shorter; it was read again from the start',
    readError: 'The file could not be read',
  };

  let { problems, anchor, open, onclose }: { problems: Problem[]; anchor: HTMLElement | undefined; open: boolean; onclose: () => void } = $props();
</script>

<Popover {anchor} {open} {onclose} placement="top-start" label="Problems" width={460}>
  <div class="list">
    <h3 class="t-label">Problems</h3>
    {#if problems.length === 0}
      <p class="muted t-small">None yet</p>
    {:else}
      <ul>
        {#each problems.slice(0, 50) as p, i (i)}
          <li>
            <span class="num t-small muted">{formatClock(p.t)}</span>
            <span class="t-mono path" title={p.path}>{p.path}</span>
            <span class="t-small reason">{REASON[p.reason] ?? p.reason}{p.count > 1 ? ` (${p.count}×)` : ''}</span>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</Popover>

<style>
  .list { padding: var(--space-2); display: flex; flex-direction: column; gap: var(--space-2); }
  h3 { color: var(--text-muted); }
  ul { display: flex; flex-direction: column; gap: var(--space-2); }
  li { display: grid; grid-template-columns: auto 1fr; gap: 2px var(--space-2); }
  .path { overflow-wrap: anywhere; }
  .reason { grid-column: 2; color: var(--text-secondary); }
</style>
