<script lang="ts">
  // RunGroup (ux-spec 3.2): a container with a 36 px header, or, collapsed, a 248 x 72 node.
  import { Handle, Position, type NodeProps } from '@xyflow/svelte';
  import IconButton from '../../components/IconButton.svelte';
  import PhaseStepper from '../../components/PhaseStepper.svelte';
  import SegmentedProgress from '../../components/SegmentedProgress.svelte';
  import StateIcon from '../../components/StateIcon.svelte';
  import { countStates, runAgents, runTotals } from '../../data/derive';
  import { formatCount, formatDuration } from '../../format';
  import { RUN_STATUS } from '../../states';
  import { actions } from '../../stores/actions';
  import { clock } from '../../stores/clock.svelte';
  import { sel, selection } from '../../stores/selection.svelte';
  import { world } from '../../stores/world.svelte';
  import { graph } from './graphState.svelte';

  let { data }: NodeProps & { data: { sessionId: string; runId: string; collapsed: boolean } } = $props();

  const s = $derived(world.session(data.sessionId));
  const run = $derived(s?.runs[data.runId] ?? null);
  const agents = $derived(s && run ? runAgents(s, run) : []);
  const totals = $derived(s && run ? runTotals(s, run) : null);
  const counts = $derived(countStates(agents, { skipSuperseded: true }));
  const steps = $derived(
    run
      ? run.phases.map((p) => {
          const own = agents.filter((a) => a.phaseIndex === p.index && !a.superseded);
          return { title: p.title, state: p.state, done: own.filter((a) => a.state === 'done').length, total: own.length };
        })
      : [],
  );
  const elapsed = $derived(run?.startedAt != null ? formatDuration(Math.max(0, (run.endedAt ?? clock.viewTime) - run.startedAt)) : '–');
  const selected = $derived(selection.is('run', data.runId, data.sessionId));
  const running = $derived(run?.status === 'running');
</script>

<Handle type="target" position={Position.Left} isConnectable={false} class="as-handle" />
{#if run && totals}
  {@const st = RUN_STATUS[run.status]}
  <div class="run st-{st.tone}" class:collapsed={data.collapsed} class:selected class:running ondblclick={() => graph.collapsedToggle(data.runId)} role="group" aria-label="Run {run.name}, {st.label}">
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div class="head" onclick={() => actions.select(sel.run(data.sessionId, data.runId))} role="button" tabindex="0" onkeydown={(e) => e.key === 'Enter' && actions.select(sel.run(data.sessionId, data.runId))}>
      <StateIcon state={st} size={16} animated={running && graph.loops.size > 0} />
      <span class="name t-title ellipsis" title={run.name}>{run.name}</span>
      {#if !data.collapsed}
        <PhaseStepper phases={steps} />
        <span class="meta t-small num">{elapsed}</span>
        <span class="meta t-small num">{totals.agents} agents</span>
      {/if}
      <IconButton icon={data.collapsed ? 'chevron-right' : 'chevron-down'} label={data.collapsed ? 'Expand run' : 'Collapse run'} size="sm" onclick={(e) => (e.stopPropagation(), graph.collapsedToggle(data.runId))} />
    </div>
    {#if data.collapsed}
      <div class="sum t-small num">{totals.agents} agents · {run.phases.length} phases · {formatCount(totals.tokens)} tok</div>
      <div class="prog"><SegmentedProgress {counts} animated={running} /></div>
    {/if}
  </div>
{/if}

<style>
  .run {
    width: 100%;
    height: 100%;
    border: var(--border-width-strong) var(--container-border-style) var(--border-strong);
    border-radius: var(--radius-lg);
    background: color-mix(in srgb, var(--surface-panel) 40%, transparent);
  }
  .run.running { border-color: var(--state-running); }
  .collapsed { background: var(--node-fill); box-shadow: var(--shadow-1); display: flex; flex-direction: column; justify-content: center; gap: 4px; padding: 0 var(--pad-3); }
  .selected { outline: 2px solid var(--select-ring); outline-offset: 2px; }
  .head { display: flex; align-items: center; gap: var(--space-2); height: 36px; padding: 0 var(--pad-2) 0 var(--pad-3); cursor: pointer; min-width: 0; }
  .collapsed .head { height: auto; padding: 0; }
  .name { flex: 1; min-width: 0; }
  .meta { color: var(--text-secondary); white-space: nowrap; }
  .sum { color: var(--text-secondary); }
</style>
