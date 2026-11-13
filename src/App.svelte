<script lang="ts">
  // Picks the data source (Tauri inside the app, the mock scenario in a plain browser),
  // starts the stores and routes to the views.
  import { onMount } from 'svelte';
  import { createTauriSource } from './lib/data/tauriSource';
  import { MockSource, mockVariantFromUrl } from './lib/data/mockSource';
  import { inTauri } from './lib/data/source';
  import AppFrame from './lib/frame/AppFrame.svelte';
  import OverviewView from './lib/overview/OverviewView.svelte';
  import { actions, clock, settings, world, type Route } from './lib/stores';
  import { attention } from './lib/stores/attention.svelte';
  import GraphView from './lib/views/graph/GraphView.svelte';
  import Inspector from './lib/views/inspector/Inspector.svelte';
  import TimelineView from './lib/views/timeline/TimelineView.svelte';
  import WorkflowView from './lib/views/workflow/WorkflowView.svelte';

  settings.init();

  onMount(() => {
    clock.start();
    const source = inTauri() ? createTauriSource() : new MockSource({ variant: mockVariantFromUrl() });
    void world.start(source).then(() => actions.applyStoredClaudeDir());
    // The menu-bar list; picking a session there opens it here.
    void attention.start((sessionId) => actions.openSession(sessionId));
    return () => world.stop();
  });
</script>

<AppFrame>
  {#snippet view(r: Route)}
    {#if r === 'overview'}
      <OverviewView />
    {:else if r === 'graph'}
      <GraphView />
    {:else if r === 'workflow'}
      <WorkflowView />
    {:else}
      <TimelineView />
    {/if}
  {/snippet}
  {#snippet inspector()}
    <Inspector />
  {/snippet}
</AppFrame>
