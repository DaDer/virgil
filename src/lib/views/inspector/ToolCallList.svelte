<script lang="ts">
  // The newest tool calls first: elapsed time, tool name, summary; failed calls marked.
  import Icon from '../../components/Icon.svelte';
  import { formatElapsed } from '../../format';
  import type { ToolTick } from '../shared/toolHistory';

  let { calls, origin, limit = 20 }: { calls: readonly ToolTick[]; origin: number | null; limit?: number } = $props();
  const shown = $derived([...calls].slice(-limit).reverse());
</script>

<ul class="calls">
  {#each shown as c (c.toolUseId)}
    <li class="row" class:err={c.isError}>
      <span class="t num">{origin != null ? formatElapsed(c.t - origin) : '–'}</span>
      <span class="name t-mono">{c.name}</span>
      <span class="sum t-mono ellipsis" title={c.summary}>{c.summary}</span>
      {#if c.isError}<span class="icon" title="Failed"><Icon name="alert" size={12} /></span>{/if}
      {#if c.endedAt == null}<span class="pending" title="No result yet"></span>{/if}
    </li>
  {:else}
    <li class="muted t-small">None yet</li>
  {/each}
</ul>

<style>
  .calls { display: flex; flex-direction: column; }
  .row { display: flex; align-items: center; gap: var(--space-2); min-height: 22px; font-size: var(--text-xs); }
  .t { flex: none; color: var(--text-muted); width: 56px; }
  .name { flex: none; font-weight: var(--weight-semibold); font-size: var(--text-xs); }
  .sum { flex: 1; color: var(--text-secondary); font-size: var(--text-xs); }
  .icon { color: var(--state-error); display: inline-grid; }
  .err .name { color: var(--state-error-text); }
  .pending { width: 6px; height: 6px; border-radius: 50%; background: var(--state-running); flex: none; }
</style>
