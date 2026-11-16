<script lang="ts">
  // The agents and background commands the active view folded away under one parent or phase:
  // how many finished, stopped or failed, how many commands, and a way to bring them back. Reads
  // live states by id, like AgentNode.
  import { Handle, Position, type NodeProps } from '@xyflow/svelte';
  import { world } from '../../stores/world.svelte';
  import { graph } from './graphState.svelte';

  let { id, data, width }: NodeProps & { data: { sessionId: string; agentIds: string[]; taskIds?: string[]; expanded: boolean } } = $props();

  const counts = $derived.by(() => {
    const c = { done: 0, stopped: 0, failed: 0 };
    for (const agentId of data.agentIds) {
      const a = world.agent(data.sessionId, agentId);
      if (!a) continue;
      if (a.state === 'failed') c.failed += 1;
      else if (a.state === 'stopped') c.stopped += 1;
      else c.done += 1;
    }
    return c;
  });
  const commands = $derived(data.taskIds?.length ?? 0);
  const total = $derived(data.agentIds.length + commands);
  const what = $derived(
    [data.agentIds.length ? `${data.agentIds.length} ${data.agentIds.length === 1 ? 'agent' : 'agents'}` : '', commands ? `${commands} ${commands === 1 ? 'command' : 'commands'}` : '']
      .filter(Boolean)
      .join(' and '),
  );
  const narrow = $derived((width ?? 168) < 120);
  const parts = $derived(
    [counts.done ? `${counts.done} finished` : '', counts.stopped ? `${counts.stopped} stopped` : '', counts.failed ? `${counts.failed} failed` : '', commands ? `${commands} ${commands === 1 ? 'command' : 'commands'}` : '']
      .filter(Boolean)
      .join(' · '),
  );
  const toggle = (e: Event) => (e.stopPropagation(), graph.stackToggle(id));
</script>

<Handle type="target" position={Position.Left} isConnectable={false} class="as-handle" />
<button
  type="button"
  class="stack"
  class:open={data.expanded}
  class:narrow
  aria-expanded={data.expanded}
  title={data.expanded ? `Fold these ${what} away again` : `Show these ${what}`}
  onclick={toggle}
>
  {#if narrow}
    <span class="n num">{data.expanded ? '−' : '+'}{total}</span>
  {:else}
    <span class="n t-strong num">{parts || `${total} earlier`}</span>
    <span class="act t-small">{data.expanded ? 'Hide' : 'Show'}</span>
  {/if}
</button>

<style>
  .stack {
    position: relative;
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    justify-content: center;
    gap: 2px;
    padding: 0 var(--space-3);
    border: var(--border-width) dashed var(--border-strong);
    border-radius: var(--radius-md);
    background: var(--surface-sunken);
    color: var(--text-secondary);
    font: inherit;
    text-align: left;
    cursor: pointer;
    pointer-events: all;
    /* Two offset edges behind the card, so it reads as a pile. */
    box-shadow:
      3px 3px 0 -1px var(--surface-sunken),
      3px 3px 0 0 var(--border-default),
      6px 6px 0 -1px var(--surface-sunken),
      6px 6px 0 0 var(--border-subtle);
    transition: background var(--dur-1) var(--ease-out), color var(--dur-1) var(--ease-out);
  }
  .stack:hover { background: var(--surface-hover); color: var(--text-primary); }
  .stack:focus-visible { outline: 2px solid var(--focus-ring); outline-offset: 2px; }
  .stack.open { box-shadow: none; border-style: solid; }
  .stack.narrow { align-items: center; padding: 0; }
  .n { font-size: var(--text-sm); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 100%; }
  .act { color: var(--accent-text); }
</style>
