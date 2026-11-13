<script lang="ts">
  // The session root node (ux-spec 3.2, 248 x 88).
  import { Handle, Position, type NodeProps } from '@xyflow/svelte';
  import StateBadge from '../../components/StateBadge.svelte';
  import TypeGlyph from '../../components/TypeGlyph.svelte';
  import { sessionStats, sessionTitle } from '../../data/derive';
  import { projectName } from '../../format';
  import { SESSION_STATE, sessionState } from '../../states';
  import { actions } from '../../stores/actions';
  import { sel, selection } from '../../stores/selection.svelte';
  import { world } from '../../stores/world.svelte';

  let { data }: NodeProps & { data: { sessionId: string } } = $props();

  const s = $derived(world.session(data.sessionId));
  const stats = $derived(s ? sessionStats(s) : null);
  const selected = $derived(selection.is('session', data.sessionId));
</script>

{#if s && stats}
  {@const st = SESSION_STATE[sessionState(s)]}
  <button type="button" class="node st-{st.tone}" class:selected onclick={() => actions.select(sel.session(data.sessionId))} aria-label="Session {sessionTitle(s)}, {st.label}">
    <span class="bar" aria-hidden="true"></span>
    <span class="body">
      <span class="head">
        <TypeGlyph kind="session" />
        <span class="title t-strong ellipsis" title={sessionTitle(s)}>{sessionTitle(s)}</span>
      </span>
      <span class="row">
        <span class="project t-mono ellipsis" title={s.cwd ?? ''}>{projectName(s.cwd)}</span>
        <StateBadge state={st} />
      </span>
      <span class="counts t-small num">{stats.agents} {stats.agents === 1 ? 'agent' : 'agents'} · {stats.runs} {stats.runs === 1 ? 'run' : 'runs'}</span>
    </span>
  </button>
{/if}
<Handle type="source" position={Position.Right} isConnectable={false} class="as-handle" />

<style>
  .node {
    display: flex;
    width: 248px;
    height: 88px;
    text-align: left;
    background: var(--node-fill);
    border: var(--border-width-strong) solid var(--border-strong);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-1);
    color: var(--text-primary);
    transition: box-shadow var(--dur-1) var(--ease-out), transform var(--dur-1) var(--ease-out);
  }
  .node:hover { box-shadow: var(--shadow-2); }
  .selected { outline: 2px solid var(--select-ring); outline-offset: 2px; }
  .bar { flex: none; width: 4px; background: var(--st); border-radius: var(--radius-md) 0 0 var(--radius-md); }
  .body { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 4px; padding: var(--pad-2) var(--pad-3); }
  .head, .row { display: flex; align-items: center; gap: var(--space-2); min-width: 0; }
  .title { flex: 1; }
  .project { flex: 1; color: var(--text-secondary); font-size: var(--text-xs); }
  .counts { color: var(--text-secondary); }
</style>
