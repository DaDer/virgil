<script lang="ts">
  // ux-spec 3.5. Props: none (reads `selection`); picks one of the five variants. The frame
  // (InspectorFrame) draws the sticky header with glyph, title, state badge and close.
  import EmptyState from '../../components/EmptyState.svelte';
  import { parsePhaseKey } from '../../data/derive';
  import { selection } from '../../stores/selection.svelte';
  import { world } from '../../stores/world.svelte';
  import AgentInspector from './AgentInspector.svelte';
  import FileInspector from './FileInspector.svelte';
  import PhaseInspector from './PhaseInspector.svelte';
  import RunInspector from './RunInspector.svelte';
  import SessionInspector from './SessionInspector.svelte';

  const cur = $derived(selection.current);
  const session = $derived(cur ? world.session(cur.sessionId ?? (cur.kind === 'session' ? cur.id : null)) : null);
  const phaseKey = $derived(cur?.kind === 'phase' ? parsePhaseKey(cur.id) : null);
</script>

{#if !cur}
  <EmptyState icon="info" title="Nothing selected" text="Select a session, agent, run, phase or file to see details." />
{:else if !session}
  <EmptyState icon="info" title="Not available" text="The selected item is not in the current data." />
{:else if cur.kind === 'session'}
  <SessionInspector {session} />
{:else if cur.kind === 'agent'}
  {@const agent = session.agents[cur.id]}
  {#if agent}
    <AgentInspector {session} {agent} />
  {:else}
    <EmptyState icon="info" title="Agent not seen yet" text="It may appear later in this recording." />
  {/if}
{:else if cur.kind === 'run'}
  {@const run = session.runs[cur.id]}
  {#if run}<RunInspector {session} {run} />{:else}<EmptyState icon="info" title="Run not seen yet" />{/if}
{:else if cur.kind === 'phase' && phaseKey}
  {@const run = session.runs[phaseKey.runId]}
  {#if run}<PhaseInspector {session} {run} index={phaseKey.index} />{:else}<EmptyState icon="info" title="Run not seen yet" />{/if}
{:else if cur.kind === 'file'}
  <FileInspector {session} path={cur.id} />
{/if}
