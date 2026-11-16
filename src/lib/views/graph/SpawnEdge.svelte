<script lang="ts">
  // Spawn, run, sequence and job edges (ux-spec 3.2). "Active" (dashes flowing toward the child)
  // while the child agent, run or background command is running and inside the motion budget.
  // Job edges are thin, dashed and have no arrowhead.
  import { BaseEdge, getSmoothStepPath, type EdgeProps } from '@xyflow/svelte';
  import { world } from '../../stores/world.svelte';
  import type { EdgeKind } from './buildGraph';
  import { graph } from './graphState.svelte';

  type Data = { kind: EdgeKind; sessionId: string; agentId?: string; runId?: string; taskId?: string };
  let { id, sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, data: raw }: EdgeProps & { data?: Data } = $props();
  const data = $derived(raw ?? { kind: 'spawn' as EdgeKind, sessionId: '' });

  const path = $derived(getSmoothStepPath({ sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, borderRadius: 10 })[0]);
  const active = $derived.by(() => {
    if (data.kind === 'sequence') return false;
    if (data.taskId) return graph.jobLoops.has(data.taskId);
    if (data.agentId) return graph.loops.has(data.agentId);
    if (data.runId) return world.run(data.sessionId, data.runId)?.status === 'running' && graph.loops.size > 0;
    return false;
  });
  const highlighted = $derived(graph.chain.has(id) || graph.hoverEdges.has(id));
</script>

<BaseEdge {id} {path} class="as-edge {data.kind}{active ? ' active loops' : ''}{highlighted ? ' hl' : ''}" markerEnd={data.kind === 'sequence' || data.kind === 'job' ? undefined : 'url(#as-arrow)'} interactionWidth={0} />
