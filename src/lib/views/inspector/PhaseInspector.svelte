<script lang="ts">
  // ux-spec 3.5, Phase: details, progress, agents.
  import type { Session, WorkflowRun } from '../../contract';
  import AgentRow from '../../components/AgentRow.svelte';
  import KeyValueList, { type KeyValueItem } from '../../components/KeyValueList.svelte';
  import SegmentedProgress from '../../components/SegmentedProgress.svelte';
  import { countStates } from '../../data/derive';
  import { formatClock, shortModel } from '../../format';
  import { actions } from '../../stores/actions';
  import { clock } from '../../stores/clock.svelte';
  import { sel, selection } from '../../stores/selection.svelte';
  import { world } from '../../stores/world.svelte';
  import { runPhases } from '../graph/buildGraph';
  import { span } from '../timeline/buildRows';
  import { orderCards } from '../workflow/model';
  import InspectorSection from './InspectorSection.svelte';

  let { session, run, index }: { session: Session; run: WorkflowRun; index: number } = $props();

  const byPhase = $derived(world.agentsByPhase(session, run));
  const phase = $derived(runPhases(run, byPhase).find((p) => p.index === index) ?? null);
  const agents = $derived(orderCards(byPhase.get(index) ?? []));
  const live = $derived(agents.filter((a) => !a.superseded));
  const counts = $derived(countStates(live));
  const sp = $derived(span(agents, clock.viewTime));

  const details = $derived.by((): KeyValueItem[] => [
    { key: 'Run', value: run.name },
    { key: 'Detail', value: phase?.detail ?? '–' },
    { key: 'Model', value: phase?.model ? shortModel(phase.model) : '–' },
    { key: 'First start', value: sp.start != null ? formatClock(sp.start) : '–' },
    { key: 'Last end', value: sp.end != null && live.every((a) => a.endedAt != null) ? formatClock(sp.end) : live.length ? 'Still running' : '–' },
  ]);
</script>

<InspectorSection title="Details">
  <KeyValueList items={details} />
</InspectorSection>

<InspectorSection title="Progress" count="{counts.done}/{live.length}">
  <SegmentedProgress {counts} total={live.length} height={6} />
  <p class="t-small secondary">
    {counts.done} done · {counts.running} running · {counts.waiting + counts.queued} waiting{counts.failed ? ` · ${counts.failed} failed` : ''}{counts.stopped ? ` · ${counts.stopped} stopped` : ''}
  </p>
</InspectorSection>

<InspectorSection title="Agents" count={agents.length || undefined}>
  {#each agents as a (a.agentId)}
    <AgentRow agent={a} selected={selection.is('agent', a.agentId, session.sessionId)} dimmed={a.superseded} onselect={() => actions.select(sel.agent(session.sessionId, a.agentId))} />
  {:else}
    <p class="muted t-small">None yet</p>
  {/each}
</InspectorSection>

<style>
  .muted { color: var(--text-muted); }
</style>
