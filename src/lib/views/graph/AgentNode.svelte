<script lang="ts">
  // Svelte Flow node wrapping AgentCard (ux-spec 3.2). Reads the live agent from the world
  // store by id, so the node array does not change on progress events.
  import { Handle, Position, type NodeProps } from '@xyflow/svelte';
  import AgentCard from '../../components/AgentCard.svelte';
  import { actions } from '../../stores/actions';
  import { search } from '../../stores/search.svelte';
  import { hover, sel, selection } from '../../stores/selection.svelte';
  import { world } from '../../stores/world.svelte';
  import type { Detail } from './buildGraph';
  import { graph } from './graphState.svelte';
  import { links } from '../shared/links.svelte';
  import { route } from '../../stores/route.svelte';

  let { data }: NodeProps & { data: { sessionId: string; agentId: string; detail: Detail } } = $props();

  const agent = $derived(world.agent(data.sessionId, data.agentId));
  const selected = $derived(selection.is('agent', data.agentId, data.sessionId));
  const touch = $derived(graph.fileTouch?.get(data.agentId));
  const searchDim = $derived(!search.isMatch(data.sessionId, data.agentId));
  const fileDim = $derived(graph.fileTouch != null && !touch);
  const hoverDim = $derived(graph.related != null && !graph.related.has(data.agentId));
  const fileBadge = $derived(touch ? (touch.writes > 0 ? ('E' as const) : ('R' as const)) : undefined);
  const animate = $derived(graph.loops.has(data.agentId) && data.detail !== 'dot');

  function enter(e: PointerEvent) {
    hover.set(sel.agent(data.sessionId, data.agentId));
    graph.tip = { agentId: data.agentId, x: e.clientX, y: e.clientY };
  }
  function leave() {
    hover.clear();
    graph.tip = null;
  }
</script>

<Handle type="target" position={Position.Left} isConnectable={false} class="as-handle" />
{#if agent}
  <div class="wrap" class:ring={!!touch} class:file-dim={fileDim} class:half={hoverDim && !fileDim && !searchDim} onpointerenter={enter} onpointerleave={leave} role="presentation">
    <AgentCard
      {agent}
      variant="node"
      detail={data.detail}
      {selected}
      dimmed={searchDim}
      {animate}
      badges={{ file: fileBadge }}
      onclick={() => actions.select(sel.agent(data.sessionId, data.agentId))}
      ondblclick={() => {
        links.focusInTimeline(data.sessionId, data.agentId);
        route.navigate('timeline');
      }}
    />
  </div>
{/if}
<Handle type="source" position={Position.Right} isConnectable={false} class="as-handle" />

<style>
  .wrap { border-radius: var(--radius-md); transition: opacity var(--dur-1) var(--ease-out); }
  .half { opacity: 0.55; }
  .file-dim { opacity: 0.4; }
  .ring { outline: 2px solid var(--accent); outline-offset: 4px; }
</style>
