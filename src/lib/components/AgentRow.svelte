<script lang="ts">
  // One-line agent for lists: state icon, type glyph, label, model letter, optional trailing text.
  import type { Snippet } from 'svelte';
  import type { Agent } from '../contract';
  import { hover, sel } from '../stores/selection.svelte';
  import ModelChip from './ModelChip.svelte';
  import StateIcon from './StateIcon.svelte';
  import TypeGlyph from './TypeGlyph.svelte';

  let {
    agent,
    selected = false,
    onselect,
    trailing,
    dimmed = false,
  }: { agent: Agent; selected?: boolean; onselect?: (a: Agent) => void; trailing?: Snippet; dimmed?: boolean } = $props();

  const hovered = $derived(hover.is('agent', agent.agentId, agent.sessionId));
</script>

<button
  type="button"
  class="row"
  class:selected
  class:hovered
  class:dimmed
  onclick={() => onselect?.(agent)}
  onpointerenter={() => hover.set(sel.agent(agent.sessionId, agent.agentId))}
  onpointerleave={() => hover.clear()}
  aria-current={selected ? 'true' : undefined}
>
  <StateIcon state={agent.state} size={14} />
  <TypeGlyph kind={agent.kind} agentType={agent.agentType} size={18} />
  <span class="label ellipsis" title={agent.label}>{agent.label}</span>
  {#if trailing}<span class="trail t-small">{@render trailing()}</span>{/if}
  <ModelChip model={agent.model} compact />
</button>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    width: 100%;
    height: var(--size-row);
    padding: 0 var(--pad-2);
    border-radius: var(--radius-sm);
    text-align: left;
    color: var(--text-primary);
    transition: background-color var(--dur-1) var(--ease-out), opacity var(--dur-1) var(--ease-out);
  }
  .row:hover, .hovered { background: var(--surface-hover); }
  .hovered { box-shadow: inset 0 0 0 1px var(--border-strong); }
  .selected { background: var(--surface-selected); box-shadow: inset 0 0 0 2px var(--select-ring); }
  .dimmed { opacity: 0.25; }
  .label { flex: 1; font-weight: var(--weight-medium); }
  .trail { color: var(--text-muted); white-space: nowrap; }
</style>
