<script lang="ts">
  // PhaseGroup (ux-spec 3.2): number, title, done/total, progress bar across the top edge.
  import { Handle, Position, type NodeProps } from '@xyflow/svelte';
  import SegmentedProgress from '../../components/SegmentedProgress.svelte';
  import { countStates } from '../../data/derive';
  import { actions } from '../../stores/actions';
  import { sel, selection } from '../../stores/selection.svelte';
  import { world } from '../../stores/world.svelte';
  import { runPhases } from './buildGraph';

  let { data }: NodeProps & { data: { sessionId: string; runId: string; index: number } } = $props();

  const s = $derived(world.session(data.sessionId));
  const run = $derived(s?.runs[data.runId] ?? null);
  const agents = $derived(s && run ? world.agentsByPhase(s, run).get(data.index) ?? [] : []);
  const phase = $derived(run ? runPhases(run, world.agentsByPhase(s!, run)).find((p) => p.index === data.index) ?? null : null);
  const counts = $derived(countStates(agents, { skipSuperseded: true }));
  const live = $derived(agents.filter((a) => !a.superseded));
  const selected = $derived(selection.is('phase', `${data.runId}:${data.index}`, data.sessionId));
  const select = () => actions.select(sel.phase(data.sessionId, data.runId, data.index));
</script>

<Handle type="target" position={Position.Left} isConnectable={false} class="as-handle" />
{#if phase}
  <div class="phase" class:selected class:active={phase.state === 'running'}>
    <div class="prog"><SegmentedProgress {counts} total={live.length} height={4} animated={false} /></div>
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div class="head" onclick={select} role="button" tabindex="0" onkeydown={(e) => e.key === 'Enter' && select()}>
      <span class="n num">{phase.index}</span>
      <span class="title t-strong ellipsis" title={phase.detail ?? phase.title}>{phase.title}</span>
      <span class="count t-small num">{counts.done}/{live.length}</span>
    </div>
    {#if !agents.length}<p class="empty t-small">No agents yet</p>{/if}
  </div>
{/if}
<Handle type="source" position={Position.Right} isConnectable={false} class="as-handle" />

<style>
  .phase {
    position: relative;
    width: 100%;
    height: 100%;
    border: var(--border-width) var(--container-border-style) var(--border-default);
    border-radius: var(--radius-md);
    background: color-mix(in srgb, var(--surface-panel) 60%, transparent);
    overflow: hidden;
  }
  .selected { outline: 2px solid var(--select-ring); outline-offset: 2px; overflow: visible; }
  .prog { position: absolute; inset: 0 0 auto 0; }
  .head { display: flex; align-items: center; gap: var(--space-2); height: 28px; padding: 4px var(--pad-2) 0; cursor: pointer; }
  .n {
    display: inline-grid; place-items: center; flex: none;
    width: 18px; height: 18px; border-radius: 50%;
    background: var(--surface-sunken); color: var(--text-secondary); font-size: var(--text-xs);
  }
  .title { flex: 1; }
  .count { color: var(--text-secondary); }
  .empty { color: var(--text-muted); padding: 4px var(--pad-2); }
</style>
