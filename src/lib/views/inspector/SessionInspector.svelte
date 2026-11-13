<script lang="ts">
  // ux-spec 3.5, Session: details, counts, runs, direct agents, activity.
  import { MAIN_AGENT_ID, type Session } from '../../contract';
  import AgentRow from '../../components/AgentRow.svelte';
  import KeyValueList, { type KeyValueItem } from '../../components/KeyValueList.svelte';
  import SegmentedProgress from '../../components/SegmentedProgress.svelte';
  import Sparkline from '../../components/Sparkline.svelte';
  import StateIcon from '../../components/StateIcon.svelte';
  import StatTile from '../../components/StatTile.svelte';
  import { bucketize, countStates, lastActivity, sessionStats, subagents } from '../../data/derive';
  import { formatClock, formatCount, formatDuration, formatInt, formatRelative } from '../../format';
  import { RUN_STATUS, SESSION_STATE, sessionState } from '../../states';
  import { actions } from '../../stores/actions';
  import { clock } from '../../stores/clock.svelte';
  import { sel, selection } from '../../stores/selection.svelte';
  import { world } from '../../stores/world.svelte';
  import InspectorSection from './InspectorSection.svelte';

  let { session }: { session: Session } = $props();

  const now = $derived(clock.viewTime);
  const stats = $derived(sessionStats(session));
  const counts = $derived(countStates(subagents(session)));
  const runs = $derived(Object.values(session.runs).sort((a, b) => (b.startedAt ?? 0) - (a.startedAt ?? 0)));
  const direct = $derived(world.childrenOf(session).get(MAIN_AGENT_ID) ?? []);
  const live = $derived(sessionState(session) !== 'stale');
  const activity = $derived.by(() => {
    const times = world.activityOf(session.sessionId);
    if (live) return { values: bucketize(times, now - 30 * 60_000, now + 1, 30), from: now - 30 * 60_000, to: now };
    const from = session.startedAt ?? times[0] ?? now;
    const to = lastActivity(session);
    return { values: bucketize(times, from, to + 1, 30), from, to };
  });

  const details = $derived.by((): KeyValueItem[] => [
    { key: 'State', value: SESSION_STATE[sessionState(session)].label },
    { key: 'Project', value: session.cwd ?? '–', mono: true, copy: !!session.cwd },
    { key: 'Session id', value: session.sessionId, mono: true, copy: true },
    { key: 'Version', value: session.version ?? '–' },
    { key: 'Started', value: session.startedAt != null ? `${formatClock(session.startedAt)} · ${formatRelative(session.startedAt, now)}` : '–' },
    { key: 'Last activity', value: formatRelative(lastActivity(session), now) },
    { key: 'Process', value: session.pid != null ? `${session.pid} · ${session.liveness === 'alive' ? 'alive' : session.liveness === 'stale' ? 'not running' : 'unknown (replay)'}` : '–', mono: true },
  ]);
</script>

<InspectorSection title="Details">
  <KeyValueList items={details} />
</InspectorSection>

<InspectorSection title="Counts">
  <SegmentedProgress {counts} height={6} label="Agents by state" />
  <div class="tiles">
    <StatTile label="Agents" value={formatInt(stats.agents)} sub={stats.running ? `${stats.running} running` : undefined} />
    <StatTile label="Runs" value={formatInt(stats.runs)} />
    <StatTile label="Tokens" value={formatCount(stats.tokens)} />
    <StatTile label="Tool calls" value={formatInt(stats.toolCalls)} />
  </div>
</InspectorSection>

<InspectorSection title="Runs" count={runs.length || undefined}>
  {#each runs as r (r.runId)}
    <button type="button" class="runrow" class:selected={selection.is('run', r.runId, session.sessionId)} onclick={() => actions.select(sel.run(session.sessionId, r.runId))}>
      <StateIcon state={RUN_STATUS[r.status]} size={14} />
      <span class="name ellipsis">{r.name}</span>
      <span class="meta t-small num">{r.startedAt != null ? formatClock(r.startedAt, false) : '–'}</span>
      <span class="meta t-small num">{r.startedAt != null ? formatDuration((r.endedAt ?? now) - r.startedAt) : '–'}</span>
      <span class="meta t-small num">{r.agentIds.length}</span>
    </button>
  {:else}
    <p class="muted t-small">None yet</p>
  {/each}
</InspectorSection>

<InspectorSection title="Agents" count={direct.length || undefined}>
  {#each direct.slice(0, 50) as a (a.agentId)}
    <AgentRow agent={a} selected={selection.is('agent', a.agentId, session.sessionId)} onselect={() => actions.select(sel.agent(session.sessionId, a.agentId))} />
  {:else}
    <p class="muted t-small">None yet</p>
  {/each}
  {#if direct.length > 50}<p class="muted t-small">and {direct.length - 50} more</p>{/if}
</InspectorSection>

<InspectorSection title="Activity">
  <Sparkline values={activity.values} width={320} height={48} highlightLast={sessionState(session) === 'busy' ? 2 : 0} label="Tool calls over time" />
  <div class="axis t-small num"><span>{live ? '30m ago' : formatClock(activity.from, false)}</span><span>{live ? 'now' : formatClock(activity.to, false)}</span></div>
</InspectorSection>

<style>
  .tiles { display: grid; grid-template-columns: 1fr 1fr; gap: var(--space-2); }
  .runrow { display: flex; align-items: center; gap: var(--space-2); width: 100%; height: var(--size-row); padding: 0 var(--pad-2); border-radius: var(--radius-sm); text-align: left; color: var(--text-primary); }
  .runrow:hover { background: var(--surface-hover); }
  .runrow.selected { background: var(--surface-selected); box-shadow: inset 0 0 0 2px var(--select-ring); }
  .name { flex: 1; font-weight: var(--weight-medium); }
  .meta { color: var(--text-muted); white-space: nowrap; }
  .muted { color: var(--text-muted); }
  .axis { display: flex; justify-content: space-between; color: var(--text-muted); width: 320px; max-width: 100%; }
</style>
