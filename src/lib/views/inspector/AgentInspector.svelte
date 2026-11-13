<script lang="ts">
  // ux-spec 3.5, Agent: chain, task, details, counts, files, recent tool calls, result, children.
  import type { Agent, Session } from '../../contract';
  import AgentRow from '../../components/AgentRow.svelte';
  import ClampText from '../../components/ClampText.svelte';
  import KeyValueList, { type KeyValueItem } from '../../components/KeyValueList.svelte';
  import StatTile from '../../components/StatTile.svelte';
  import { parentChain } from '../../data/derive';
  import { formatClock, formatCount, formatDuration, formatInt, formatRelative, shortModel } from '../../format';
  import { agentTypeMeta, AGENT_STATE } from '../../states';
  import { actions } from '../../stores/actions';
  import { clock } from '../../stores/clock.svelte';
  import { sel, selection } from '../../stores/selection.svelte';
  import { world } from '../../stores/world.svelte';
  import { history } from '../shared/history.svelte';
  import { ticksUntil, toolBreakdown } from '../shared/toolHistory';
  import FileList from './FileList.svelte';
  import InspectorSection from './InspectorSection.svelte';
  import { agentFiles, isPreview, peerMax } from './model';
  import ParentChain from './ParentChain.svelte';
  import ToolBreakdown from './ToolBreakdown.svelte';
  import ToolCallList from './ToolCallList.svelte';

  let { session, agent }: { session: Session; agent: Agent } = $props();

  const now = $derived(clock.viewTime);
  const chain = $derived(parentChain(session, agent.agentId));
  const files = $derived(agentFiles(session, agent.agentId));
  const calls = $derived.by(() => {
    const all = history.ticks(session.sessionId, agent.agentId);
    return world.mode === 'replay' ? ticksUntil(all, now) : all;
  });
  const breakdown = $derived(toolBreakdown(calls));
  const peers = $derived(peerMax(session, agent));
  const children = $derived(world.childrenOf(session).get(agent.agentId) ?? []);
  const run = $derived(agent.runId ? session.runs[agent.runId] : null);
  const phase = $derived(run?.phases.find((p) => p.index === agent.phaseIndex));
  const active = $derived(agent.state === 'running' || agent.state === 'waiting' || agent.state === 'queued');

  const details = $derived.by((): KeyValueItem[] => {
    const a = agent;
    const items: KeyValueItem[] = [
      { key: 'State', value: `${AGENT_STATE[a.state].label}${a.stateReason ? ` (${a.stateReason === 'backgroundWork' ? 'paused while its own background work runs' : a.stateReason})` : ''}` },
      { key: 'Type', value: agentTypeMeta(a.agentType, a.kind).label },
      { key: 'Model', value: a.model ? `${shortModel(a.model)}` : '–', title: a.model ?? undefined },
      { key: 'Started', value: a.startedAt != null ? `${formatClock(a.startedAt)} · ${formatRelative(a.startedAt, now)}` : '–' },
      { key: 'Waited', value: a.queuedAt != null && a.startedAt != null ? formatDuration(a.startedAt - a.queuedAt) : '–' },
      { key: 'Ran', value: a.startedAt != null ? formatDuration(Math.max(0, (a.endedAt ?? now) - a.startedAt)) : '–' },
    ];
    if (a.phaseIndex != null) items.push({ key: 'Phase', value: `${a.phaseIndex} ${phase?.title ?? a.phaseTitle ?? ''}`.trim() });
    if (a.attempt != null) items.push({ key: 'Attempt', value: String(a.attempt) });
    if (a.worktreePath) items.push({ key: 'Worktree', value: a.worktreePath, mono: true, copy: true });
    items.push({ key: 'Agent id', value: a.agentId, mono: true, copy: true });
    return items;
  });
</script>

<ParentChain {chain} sessionId={session.sessionId} />

<InspectorSection title="Task">
  {#if agent.task}
    <ClampText text={agent.task} lines={12} />
    {#if isPreview(agent.task)}<p class="note t-small">Preview only</p>{/if}
  {:else}
    <p class="muted t-small">None yet</p>
  {/if}
</InspectorSection>

<InspectorSection title="Details">
  <KeyValueList items={details} />
</InspectorSection>

<InspectorSection title="Counts">
  <div class="tiles">
    <StatTile label="Tokens" value={agent.tokens > 0 ? formatCount(agent.tokens) : '–'} sub={agent.outputTokens > 0 ? `${formatCount(agent.outputTokens)} out` : undefined} share={peers.tokens ? agent.tokens / peers.tokens : 0} />
    <StatTile label="Tool calls" value={formatInt(agent.toolCalls)} share={peers.toolCalls ? agent.toolCalls / peers.toolCalls : 0} />
  </div>
  {#if breakdown.length}<ToolBreakdown counts={breakdown} />{/if}
</InspectorSection>

<InspectorSection title="Files touched" count={files.length || undefined}>
  {#if files.length}
    <FileList {files} cwd={session.cwd} onselect={(p) => actions.select(sel.file(session.sessionId, p))} />
    {#if files.some((f) => f.viaBash)}<p class="note t-small">Paths from shell commands are a hint, not a complete list.</p>{/if}
  {:else}
    <p class="muted t-small">No file reads or edits seen. Shell commands are not tracked.</p>
  {/if}
</InspectorSection>

<InspectorSection title="Recent tool calls" count={calls.length || undefined}>
  <ToolCallList {calls} origin={agent.startedAt} />
</InspectorSection>

<InspectorSection title="Result">
  {#if agent.state === 'failed'}
    <p class="error t-small">{agent.stateReason ?? 'Failed'}{agent.superseded ? ' (replaced by a retry)' : ''}</p>
  {/if}
  {#if agent.resultPreview}
    <ClampText text={agent.resultPreview} lines={12} />
  {:else if active}
    <p class="muted t-small">Still running</p>
  {:else if agent.state !== 'failed'}
    <p class="muted t-small">None yet</p>
  {/if}
</InspectorSection>

{#if children.length}
  <InspectorSection title="Started agents" count={children.length}>
    <div class="list">
      {#each children as c (c.agentId)}
        <AgentRow agent={c} selected={selection.is('agent', c.agentId, session.sessionId)} onselect={() => actions.select(sel.agent(session.sessionId, c.agentId))} />
      {/each}
    </div>
  </InspectorSection>
{/if}

<style>
  .tiles { display: grid; grid-template-columns: 1fr 1fr; gap: var(--space-2); }
  .note { color: var(--text-muted); }
  .muted { color: var(--text-muted); }
  .error { padding: var(--space-2) var(--space-3); border-radius: var(--radius-sm); background: var(--state-error-soft); color: var(--state-error-text); white-space: pre-wrap; overflow-wrap: anywhere; }
  .list { display: flex; flex-direction: column; }
</style>
