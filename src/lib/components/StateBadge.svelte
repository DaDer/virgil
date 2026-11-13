<script lang="ts">
  // Icon + word on the state's soft colour (ux-spec 4.6).
  import type { AgentState } from '../contract';
  import { AGENT_STATE, type StateMeta } from '../states';
  import StateIcon from './StateIcon.svelte';

  let { state, label }: { state: AgentState | StateMeta; label?: string } = $props();
  const meta = $derived(typeof state === 'string' ? AGENT_STATE[state] : state);
</script>

<span class="badge st-{meta.tone}">
  <StateIcon {state} size={14} />
  <span class="t-label">{label ?? meta.label}</span>
</span>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    height: 22px;
    padding: 0 var(--space-2) 0 var(--space-1);
    border-radius: var(--radius-pill);
    background: var(--st-soft);
    color: var(--st-text);
    white-space: nowrap;
    transition: background-color var(--dur-2) var(--ease-out), color var(--dur-2) var(--ease-out);
  }
</style>
