<script lang="ts" module>
  export type AgentCardBadges = { attempt?: number | null; children?: number; file?: 'R' | 'E' };
</script>

<script lang="ts">
  // The one visual for an agent (ux-spec 3.2 AgentNode, 3.3 AgentCard). Graph uses
  // variant="node", Workflow uses variant="card". Reads the clock for timers; never "now".
  import type { Agent } from '../contract';
  import { formatCount, formatDuration, taskSummary } from '../format';
  import { AGENT_STATE, isActive } from '../states';
  import { clock } from '../stores/clock.svelte';
  import Icon from './Icon.svelte';
  import ModelChip from './ModelChip.svelte';
  import StateIcon from './StateIcon.svelte';
  import TypeGlyph from './TypeGlyph.svelte';

  let {
    agent,
    variant = 'card',
    detail = 'full',
    selected = false,
    dimmed = false,
    badges = {},
    animate = true,
    onclick,
    ondblclick,
  }: {
    agent: Agent;
    variant?: 'node' | 'card';
    detail?: 'full' | 'compact' | 'dot';
    selected?: boolean;
    dimmed?: boolean;
    badges?: AgentCardBadges;
    /** Views turn this off for agents outside the motion budget (ux-spec 5.3). */
    animate?: boolean;
    onclick?: (e: MouseEvent) => void;
    ondblclick?: (e: MouseEvent) => void;
  } = $props();

  const meta = $derived(AGENT_STATE[agent.state]);
  const running = $derived(agent.state === 'running');
  const active = $derived(isActive(agent.state));
  const summary = $derived(taskSummary(agent.task));
  const attempt = $derived(badges.attempt ?? agent.attempt);
  const now = $derived(clock.viewTime);

  const timer = $derived.by(() => {
    if (agent.state === 'waiting') return `Waiting ${formatDuration(Math.max(0, now - (agent.lastActivityAt ?? agent.startedAt ?? now)))}`;
    if (agent.state === 'queued') return agent.queuedAt != null ? `Queued ${formatDuration(Math.max(0, now - agent.queuedAt))}` : 'Queued';
    if (agent.startedAt == null) return '';
    return formatDuration(Math.max(0, (agent.endedAt ?? now) - agent.startedAt));
  });

  const toolLine = $derived.by(() => {
    if (!active && agent.startedAt != null && agent.endedAt != null) return { kind: 'took' as const, text: `Took ${formatDuration(agent.endedAt - agent.startedAt)}` };
    if (!agent.lastTool) return { kind: 'start' as const, text: active ? 'Starting…' : 'No tool calls' };
    return { kind: 'tool' as const, name: agent.lastTool.name, text: agent.lastTool.summary };
  });

  // One-shot pulse when a tool call arrives: at most once per 800 ms, never above 10x replay
  // or while scrubbing (ux-spec 5.3, rule 5).
  let pulse = $state(0);
  let lastCalls = -1;
  let lastPulse = 0;
  $effect(() => {
    const n = agent.toolCalls;
    if (lastCalls >= 0 && n > lastCalls && animate && detail !== 'dot') {
      const t = Date.now();
      const fast = clock.mode === 'replay' && (clock.speed > 10 || clock.scrubbing != null);
      if (!fast && t - lastPulse >= 800) {
        lastPulse = t;
        pulse += 1;
      }
    }
    lastCalls = n;
  });

  const interactive = $derived(!!onclick);
  const aria = $derived(`${agent.label}, ${meta.label}${agent.model ? `, ${agent.model}` : ''}`);
</script>

{#if detail === 'dot'}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div
    class="dot st-{meta.tone}"
    class:selected
    class:dimmed
    title="{agent.label}: {meta.label}"
    role={interactive ? 'button' : 'img'}
    tabindex={interactive ? 0 : undefined}
    aria-label={aria}
    {onclick}
    {ondblclick}
    onkeydown={(e) => interactive && (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), onclick?.(e as unknown as MouseEvent))}
  >
    <StateIcon state={agent.state} size={16} animated={false} />
  </div>
{:else}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div
    class="card {variant} {detail} st-{meta.tone}"
    class:running
    class:failed={agent.state === 'failed'}
    class:waiting={agent.state === 'waiting' || agent.state === 'queued'}
    class:superseded={agent.superseded}
    class:selected
    class:dimmed
    class:interactive
    role={interactive ? 'button' : 'group'}
    tabindex={interactive ? 0 : undefined}
    aria-label={aria}
    aria-pressed={interactive ? selected : undefined}
    {onclick}
    {ondblclick}
    onkeydown={(e) => interactive && (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), onclick?.(e as unknown as MouseEvent))}
  >
    {#if running && variant === 'node' && animate}<span class="breathe loops" aria-hidden="true"></span>{/if}
    <span class="bar" aria-hidden="true"></span>
    <div class="body">
      <div class="head">
        {#if detail === 'compact'}<span class="icon-wrap"><StateIcon state={agent.state} size={14} animated={animate} />{#key pulse}{#if pulse}<span class="ping"></span>{/if}{/key}</span>{/if}
        <TypeGlyph kind={agent.kind} agentType={agent.agentType} size={detail === 'compact' ? 18 : 20} />
        <span class="label ellipsis" title={agent.label}>{agent.label}</span>
        {#if attempt != null && attempt > 1}<span class="mini" title="Attempt {attempt}">x{attempt}</span>{/if}
        {#if agent.worktreePath}<span class="mini icon" title="Worktree: {agent.worktreePath}"><Icon name="branch" size={12} /></span>{/if}
        {#if badges.file}<span class="mini file" title={badges.file === 'E' ? 'Edited the file' : 'Read the file'}>{badges.file}</span>{/if}
        {#if detail === 'compact'}
          <ModelChip model={agent.model} compact />
        {:else}
          {#if variant === 'card' && timer}<span class="timer num t-small">{timer}</span>{/if}
          <span class="icon-wrap"><StateIcon state={agent.state} size={16} animated={animate} />{#key pulse}{#if pulse}<span class="ping"></span>{/if}{/key}</span>
        {/if}
      </div>
      {#if detail === 'full'}
        <p class="task t-small" title={agent.task || undefined}>{summary || '–'}</p>
      {/if}
      {#if detail === 'full' || variant === 'card'}
        <div class="meta t-small">
          {#if detail !== 'compact'}<ModelChip model={agent.model} />{:else if timer}<span class="num">{timer}</span>{/if}
          <span class="num" title="Context tokens">{agent.tokens > 0 ? `${formatCount(agent.tokens)} tok` : '– tok'}</span>
          <span class="num" title="Tool calls">{agent.toolCalls} {agent.toolCalls === 1 ? 'tool' : 'tools'}</span>
        </div>
      {/if}
      {#if detail === 'full'}
        <div class="tool t-mono" class:muted={toolLine.kind !== 'tool'}>
          {#if toolLine.kind === 'tool'}
            <span class="chev" aria-hidden="true">›</span><span class="tname">{toolLine.name}</span>
            <span class="tsum ellipsis">{toolLine.text}</span>
          {:else}
            <span class="ellipsis">{toolLine.text}</span>
          {/if}
        </div>
      {/if}
    </div>
    {#if badges.children}<span class="stack num" title="{badges.children} collapsed agents">+{badges.children}</span>{/if}
  </div>
{/if}

<style>
  .card {
    position: relative;
    display: flex;
    min-width: 0;
    background: var(--node-fill);
    color: var(--text-primary);
    border: var(--border-width) solid var(--border-default);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-1);
    transition:
      border-color var(--dur-2) var(--ease-out),
      background-color var(--dur-2) var(--ease-out),
      opacity var(--dur-1) var(--ease-out),
      transform var(--dur-1) var(--ease-out),
      box-shadow var(--dur-1) var(--ease-out);
    animation: as-appear var(--dur-2) var(--ease-out);
  }
  .node.full { width: 248px; height: 104px; }
  .node.compact { width: 168px; height: 44px; }
  .card.card.full { min-height: 96px; }
  .card.card.compact { min-height: 56px; }
  .running { border-color: var(--state-running); }
  .failed { border-color: var(--state-error); background: var(--state-error-soft); }
  .waiting { opacity: 0.6; }
  .superseded { opacity: 0.5; }
  .dimmed { opacity: 0.25; }
  .interactive { cursor: pointer; }
  .interactive:hover { box-shadow: var(--shadow-2); transform: translateY(-1px); }
  :global(:root[data-theme='poster']) .interactive:hover { transform: translate(-1px, -1px); }
  :global(:root[data-theme='poster']) .interactive:active { transform: translate(2px, 2px); box-shadow: var(--shadow-0); }
  .selected { outline: 2px solid var(--select-ring); outline-offset: 2px; }

  .bar {
    flex: none;
    width: 4px;
    border-radius: var(--radius-md) 0 0 var(--radius-md);
    margin: calc(-1 * var(--border-width)) 0 calc(-1 * var(--border-width)) calc(-1 * var(--border-width));
    background: var(--st);
    transition: background-color var(--dur-2) var(--ease-out);
  }
  .breathe {
    position: absolute;
    inset: -3px;
    border-radius: calc(var(--radius-md) + 3px);
    border: 2px solid var(--state-running);
    pointer-events: none;
    animation: as-breathe var(--dur-loop) var(--ease-in-out) infinite alternate;
  }
  :global(:root[data-motion='reduced']) .breathe { opacity: 1; }

  .body { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 3px; padding: var(--pad-2) var(--pad-3) var(--pad-2) var(--pad-2); }
  .compact .body { justify-content: center; }
  .head { display: flex; align-items: center; gap: var(--space-2); min-width: 0; }
  .label { flex: 1; font-weight: var(--weight-semibold); }
  .timer { color: var(--text-secondary); white-space: nowrap; }
  .task {
    color: var(--text-secondary);
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    overflow: hidden;
    overflow-wrap: anywhere;
  }
  .node .task { height: calc(var(--leading-sm) * 2); }
  .meta { display: flex; align-items: center; gap: var(--space-3); color: var(--text-secondary); min-width: 0; }
  .meta :global(.chip) { max-width: 45%; }
  .tool { display: flex; align-items: baseline; gap: 6px; min-width: 0; color: var(--text-primary); font-size: var(--text-xs); }
  .tool.muted { color: var(--text-muted); }
  .chev { color: var(--text-muted); }
  .tname { font-weight: var(--weight-semibold); flex: none; }
  .tsum { color: var(--text-secondary); }
  .mini {
    flex: none;
    display: inline-grid;
    place-items: center;
    min-width: 18px;
    height: 16px;
    padding: 0 3px;
    border-radius: var(--radius-xs);
    background: var(--surface-sunken);
    color: var(--text-secondary);
    font-size: var(--text-xs);
    font-weight: var(--weight-semibold);
  }
  .mini.file { background: var(--accent-soft); color: var(--accent-text); }
  .stack {
    position: absolute;
    right: -10px;
    top: 50%;
    transform: translateY(-50%);
    padding: 1px 5px;
    border-radius: var(--radius-pill);
    background: var(--surface-raised);
    border: var(--border-width) solid var(--border-strong);
    font-size: var(--text-xs);
    color: var(--text-secondary);
  }
  .icon-wrap { position: relative; display: inline-grid; place-items: center; flex: none; }
  .ping {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    border: 1.5px solid var(--st);
    animation: as-ping var(--dur-4) var(--ease-out) forwards;
    pointer-events: none;
  }
  :global(:root[data-motion='reduced']) .ping { display: none; }

  .dot {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: var(--st);
    transition: background-color var(--dur-2) var(--ease-out), opacity var(--dur-1) var(--ease-out);
  }
  .dot :global(.state-icon) { stroke: var(--surface-card); }
  .dot :global(.state-icon .dot), .dot :global(.state-icon .fill) { fill: var(--surface-card); }
  .dot.selected { outline: 2px solid var(--select-ring); outline-offset: 2px; }
  .dot.dimmed { opacity: 0.25; }
</style>
