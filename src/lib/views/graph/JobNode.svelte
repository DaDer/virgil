<script lang="ts" module>
  import type { BackgroundTask } from '../../contract';

  /** The one-line name of a background command; the summary can be empty. */
  export function jobLabel(t: Pick<BackgroundTask, 'summary'>): string {
    return t.summary.trim() || 'Unnamed command';
  }
</script>

<script lang="ts">
  // A background command (ux-spec 3.2): a small pill hanging off the agent that started it.
  // Reads the live task by id, like AgentNode, so a status change does not relayout. Not
  // selectable on its own: a click selects the agent (or session) that started it.
  import { Handle, Position, type NodeProps } from '@xyflow/svelte';
  import Icon from '../../components/Icon.svelte';
  import StateIcon from '../../components/StateIcon.svelte';
  import { MAIN_AGENT_ID } from '../../contract';
  import { formatDuration } from '../../format';
  import { BACKGROUND_STATUS } from '../../states';
  import { actions } from '../../stores/actions';
  import { clock } from '../../stores/clock.svelte';
  import { sel } from '../../stores/selection.svelte';
  import { world } from '../../stores/world.svelte';
  import type { Detail } from './buildGraph';
  import { graph } from './graphState.svelte';

  let { data }: NodeProps & { data: { sessionId: string; taskId: string; detail: Detail } } = $props();

  const task = $derived(world.backgroundTask(data.sessionId, data.taskId));
  const meta = $derived(task ? BACKGROUND_STATUS[task.status] : null);
  const label = $derived(task ? jobLabel(task) : '');
  const time = $derived.by(() => {
    if (!task) return '';
    const d = formatDuration(Math.max(0, (task.endedAt ?? clock.viewTime) - task.startedAt));
    return task.endedAt != null ? `Took ${d}` : d;
  });
  const running = $derived(task?.status === 'running');
  const animate = $derived(graph.jobLoops.has(data.taskId));
  // Like ended agents in the active view; an unknown outcome is always drawn faded.
  const ended = $derived(!!task && (task.status === 'unknown' || (graph.fadeEnded && (task.status === 'completed' || task.status === 'stopped'))));
  const hoverDim = $derived(graph.related != null && !!task && !graph.related.has(task.agentId));

  function enter(e: PointerEvent) {
    graph.jobTip = { taskId: data.taskId, x: e.clientX, y: e.clientY };
  }
  function leave() {
    graph.jobTip = null;
  }
  function select() {
    if (!task) return;
    actions.select(task.agentId === MAIN_AGENT_ID ? sel.session(data.sessionId) : sel.agent(data.sessionId, task.agentId));
  }
</script>

<Handle type="target" position={Position.Left} isConnectable={false} class="as-handle" />
{#if task && meta}
  <button
    type="button"
    class="job st-{meta.tone}"
    class:dot={data.detail === 'dot'}
    class:running
    class:failed={task.status === 'failed'}
    class:ended
    class:half={hoverDim}
    aria-label="Background command {label}, {meta.label}. Selects the agent that started it."
    onclick={select}
    onpointerenter={enter}
    onpointerleave={leave}
  >
    {#if data.detail === 'dot'}
      <Icon name="terminal" size={14} />
    {:else}
      <Icon name="terminal" size={14} class="glyph" />
      <span class="text">
        <span class="label ellipsis">{label}</span>
        <span class="time num">{time}</span>
      </span>
      <StateIcon state={meta} size={14} animated={animate} />
    {/if}
  </button>
{/if}

<style>
  .job {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 var(--space-2);
    background: var(--node-fill);
    color: var(--text-primary);
    border: var(--border-width) solid var(--border-default);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-1);
    font: inherit;
    font-size: var(--text-xs);
    text-align: left;
    cursor: pointer;
    pointer-events: all;
    transition:
      border-color var(--dur-2) var(--ease-out),
      background-color var(--dur-2) var(--ease-out),
      opacity var(--dur-1) var(--ease-out),
      box-shadow var(--dur-1) var(--ease-out);
    animation: as-appear var(--dur-2) var(--ease-out);
  }
  .job:hover { box-shadow: var(--shadow-2); }
  .job:focus-visible { outline: 2px solid var(--focus-ring); outline-offset: 2px; }
  .running { border-color: var(--state-running); }
  .failed { border-color: var(--state-error); background: var(--state-error-soft); }
  .ended { opacity: 0.6; }
  .ended:hover { opacity: 1; }
  .half { opacity: 0.55; }
  .job :global(.glyph) { color: var(--text-muted); }
  .text { flex: 1; min-width: 0; display: flex; flex-direction: column; line-height: var(--leading-xs); }
  .label { color: var(--text-primary); }
  .time { color: var(--text-secondary); white-space: nowrap; }

  .dot {
    justify-content: center;
    padding: 0;
    border: 0;
    border-radius: var(--radius-sm);
    background: var(--st);
    color: var(--surface-card);
  }
  .dot.failed { background: var(--state-error); }
</style>
