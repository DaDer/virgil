<script lang="ts">
  // ux-spec 3.2. Props: none (reads stores).
  import '@xyflow/svelte/dist/style.css';
  import { SvelteFlowProvider } from '@xyflow/svelte';
  import EmptyState from '../../components/EmptyState.svelte';
  import { scope } from '../../stores/route.svelte';
  import { world } from '../../stores/world.svelte';
  import GraphCanvas from './GraphCanvas.svelte';

  const session = $derived(world.session(scope.sessionId));
</script>

<section class="view" aria-label="Graph">
  {#if session}
    {#key session.sessionId}
      <SvelteFlowProvider>
        <GraphCanvas sessionId={session.sessionId} />
      </SvelteFlowProvider>
    {/key}
  {:else}
    <EmptyState icon="graph" title="No session in scope" text="Pick a session on the Overview to see its graph." />
  {/if}
</section>

<style>
  .view { height: 100%; display: grid; min-height: 0; }
</style>
