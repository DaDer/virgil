<script lang="ts">
  // Shared-file link (ux-spec 3.2 overlay): one dashed curve per pair of agents, width by the
  // number of shared files, a count label at zoom 0.6 and above. Click lists the paths.
  import { BaseEdge, EdgeLabel, getBezierPath, type EdgeProps } from '@xyflow/svelte';
  import { graph } from './graphState.svelte';

  let { id, sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, data: raw }: EdgeProps & {
    data?: { files: string[]; bothEdited: boolean };
  } = $props();
  const data = $derived(raw ?? { files: [], bothEdited: false });

  const geo = $derived(getBezierPath({ sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition }));
  const n = $derived(data.files.length);
  const width = $derived(n >= 5 ? 3 : n >= 2 ? 2 : 1);

  function open(e: MouseEvent) {
    e.stopPropagation();
    graph.linkPopover = { x: e.clientX, y: e.clientY, files: data.files };
  }
</script>

<BaseEdge {id} path={geo[0]} class="as-shared{data.bothEdited ? ' edits' : ''}" style="stroke-width: {width}px" interactionWidth={14} onclick={open} />
{#if graph.zoom >= 0.6}
  <EdgeLabel x={geo[1]} y={geo[2]} transparent>
    <button type="button" class="as-shared-label num" onclick={open} aria-label="{n} shared {n === 1 ? 'file' : 'files'}">{n}</button>
  </EdgeLabel>
{/if}
