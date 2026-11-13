<script lang="ts">
  // ux-spec 3.5, Run: summary, details, progress per phase, totals, activity, result.
  import type { Session, WorkflowRun } from '../../contract';
  import KeyValueList, { type KeyValueItem } from '../../components/KeyValueList.svelte';
  import SegmentedProgress from '../../components/SegmentedProgress.svelte';
  import StatTile from '../../components/StatTile.svelte';
  import { countStates, runTotals } from '../../data/derive';
  import { formatClock, formatCount, formatDuration, formatInt, formatRelative, shortModel } from '../../format';
  import { actions } from '../../stores/actions';
  import { clock } from '../../stores/clock.svelte';
  import { sel, selection } from '../../stores/selection.svelte';
  import { world } from '../../stores/world.svelte';
  import { runPhases } from '../graph/buildGraph';
  import { activityLines } from '../workflow/model';
  import InspectorSection from './InspectorSection.svelte';

  let { session, run }: { session: Session; run: WorkflowRun } = $props();

  const now = $derived(clock.viewTime);
  const totals = $derived(runTotals(session, run));
  const byPhase = $derived(world.agentsByPhase(session, run));
  const phases = $derived(runPhases(run, byPhase));
  const lines = $derived(activityLines(session, run, now).slice(-20));
  const results = $derived(
    run.agentIds
      .map((id) => session.agents[id])
      .filter((a) => a && a.resultPreview && !a.superseded)
      .slice(-3),
  );

  const details = $derived.by((): KeyValueItem[] => [
    { key: 'Started', value: run.startedAt != null ? `${formatClock(run.startedAt)} · ${formatRelative(run.startedAt, now)}` : '–' },
    { key: run.endedAt != null ? 'Duration' : 'Elapsed', value: run.startedAt != null ? formatDuration(Math.max(0, (run.endedAt ?? now) - run.startedAt)) : '–' },
    { key: 'Default model', value: run.defaultModel ? shortModel(run.defaultModel) : '–' },
    { key: 'Agents', value: run.declaredAgentCount != null ? `${run.declaredAgentCount}` : `${totals.agents} so far` },
    { key: 'Run id', value: run.runId, mono: true, copy: true },
    ...(run.error ? [{ key: 'Status note', value: run.error }] : []),
  ]);
</script>

{#if run.summary}
  <InspectorSection title="Summary">
    <p class="t-small summary">{run.summary}</p>
  </InspectorSection>
{/if}

<InspectorSection title="Details">
  <KeyValueList items={details} />
</InspectorSection>

<InspectorSection title="Progress" count="{totals.done}/{totals.agents}">
  {#each phases as p (p.index)}
    {@const agents = (byPhase.get(p.index) ?? []).filter((a) => !a.superseded)}
    {@const c = countStates(agents)}
    <button type="button" class="phase" class:selected={selection.is('phase', `${run.runId}:${p.index}`, session.sessionId)} onclick={() => actions.select(sel.phase(session.sessionId, run.runId, p.index))}>
      <span class="n num">{p.index}</span>
      <span class="title ellipsis">{p.title}</span>
      <span class="count t-small num">{c.done}/{agents.length}</span>
      <span class="bar"><SegmentedProgress counts={c} total={agents.length} height={4} animated={false} /></span>
    </button>
  {:else}
    <p class="muted t-small">None yet</p>
  {/each}
</InspectorSection>

<InspectorSection title="Totals">
  <div class="tiles">
    <StatTile label="Tokens" value={formatCount(totals.tokens)} sub={run.final ? undefined : 'sum so far'} />
    <StatTile label="Tool calls" value={formatInt(totals.toolCalls)} />
  </div>
</InspectorSection>

<InspectorSection title="Activity" count={lines.length || undefined}>
  <ul class="log t-mono">
    {#each lines as l, i (i)}<li><span class="muted">{formatClock(l.t)}</span> {l.text}</li>{:else}<li class="muted">None yet</li>{/each}
  </ul>
  <p class="muted t-small">The run file's log lines and script are not part of the data contract yet.</p>
</InspectorSection>

<InspectorSection title="Result">
  {#if run.status === 'running'}
    <p class="muted t-small">Still running</p>
  {:else if results.length}
    {#each results as a (a.agentId)}
      <p class="t-small res"><span class="t-strong">{a.label}</span>: {a.resultPreview}</p>
    {/each}
  {:else}
    <p class="muted t-small">None yet</p>
  {/if}
</InspectorSection>

<style>
  .summary { color: var(--text-secondary); white-space: pre-wrap; }
  .phase { display: grid; grid-template-columns: 22px 1fr auto; grid-template-rows: auto auto; align-items: center; gap: 2px var(--space-2); width: 100%; padding: 4px var(--pad-2); border-radius: var(--radius-sm); text-align: left; color: var(--text-primary); }
  .phase:hover { background: var(--surface-hover); }
  .phase.selected { background: var(--surface-selected); box-shadow: inset 0 0 0 2px var(--select-ring); }
  .n { color: var(--text-muted); font-size: var(--text-xs); }
  .count { color: var(--text-secondary); }
  .bar { grid-column: 2 / 4; }
  .tiles { display: grid; grid-template-columns: 1fr 1fr; gap: var(--space-2); }
  .log { display: flex; flex-direction: column; gap: 2px; font-size: var(--text-xs); }
  .muted { color: var(--text-muted); }
  .res { overflow-wrap: anywhere; color: var(--text-secondary); }
</style>
