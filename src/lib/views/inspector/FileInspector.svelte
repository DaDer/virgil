<script lang="ts">
  // ux-spec 3.5, File: full path, and the agents that touched it.
  import type { Session } from '../../contract';
  import StateIcon from '../../components/StateIcon.svelte';
  import { formatClock } from '../../format';
  import { actions } from '../../stores/actions';
  import { sel } from '../../stores/selection.svelte';
  import InspectorSection from './InspectorSection.svelte';
  import { fileTouchers } from './model';

  let { session, path }: { session: Session; path: string } = $props();
  const data = $derived(fileTouchers(session, path));
</script>

<p class="path t-mono">{path}</p>
{#if data.editors >= 2}<p class="warn t-small">Edited by {data.editors} agents</p>{/if}

<InspectorSection title="Touched by" count={data.touchers.length || undefined}>
  {#each data.touchers as t (t.agentId)}
    <button type="button" class="row" onclick={() => actions.select(sel.agent(session.sessionId, t.agentId))}>
      <span class="badge" class:edit={t.mode === 'E'}>{t.mode}</span>
      {#if t.agent}<StateIcon state={t.agent.state} size={14} />{/if}
      <span class="name ellipsis">{t.agent?.label ?? t.agentId}</span>
      <span class="meta t-small num" title="First and last touch">{formatClock(t.first)}{t.last !== t.first ? `–${formatClock(t.last)}` : ''}</span>
      <span class="meta t-small num">×{t.count}</span>
    </button>
  {:else}
    <p class="muted t-small">None yet</p>
  {/each}
</InspectorSection>

<style>
  .path { padding: var(--space-3) var(--space-4) 0; color: var(--text-secondary); overflow-wrap: anywhere; font-size: var(--text-xs); }
  .warn { margin: var(--space-2) var(--space-4) 0; padding: 4px var(--space-2); border-radius: var(--radius-sm); background: var(--state-queued-soft); color: var(--state-queued-text); }
  .row { display: flex; align-items: center; gap: var(--space-2); width: 100%; height: var(--size-row); padding: 0 var(--pad-2); border-radius: var(--radius-sm); text-align: left; color: var(--text-primary); }
  .row:hover { background: var(--surface-hover); }
  .badge { flex: none; display: inline-grid; place-items: center; width: 16px; height: 16px; border-radius: var(--radius-xs); background: var(--surface-sunken); color: var(--text-secondary); font-size: var(--text-xs); font-weight: var(--weight-semibold); }
  .badge.edit { background: var(--accent-soft); color: var(--accent-text); }
  .name { flex: 1; }
  .meta { color: var(--text-muted); white-space: nowrap; }
  .muted { color: var(--text-muted); }
</style>
