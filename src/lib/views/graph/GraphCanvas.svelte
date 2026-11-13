<script lang="ts">
  // The Graph (ux-spec 3.2). Structure comes from buildGraph; layout runs in ELK's worker only
  // when the structure key changes, debounced 300 ms. Node and edge arrays change only then
  // (and when the shared-file links change); live values are read by the node components.
  import { MiniMap, SvelteFlow, useSvelteFlow, type Edge, type Node, type Viewport } from '@xyflow/svelte';
  import { onDestroy, untrack } from 'svelte';
  import { MAIN_AGENT_ID, type Agent, type FileTouch } from '../../contract';
  import { taskSummary, formatDuration } from '../../format';
  import { AGENT_STATE } from '../../states';
  import { actions } from '../../stores/actions';
  import { clock } from '../../stores/clock.svelte';
  import { hover, sel, selection } from '../../stores/selection.svelte';
  import { settings } from '../../stores/settings.svelte';
  import { ui } from '../../stores/ui.svelte';
  import { world } from '../../stores/world.svelte';
  import { links } from '../shared/links.svelte';
  import { loopBudget } from '../shared/motion';
  import AgentNode from './AgentNode.svelte';
  import { buildGraph, chainEdges, SESSION_NODE, zoomBand, type DetailSetting, type GraphModel } from './buildGraph';
  import GraphToolbar from './GraphToolbar.svelte';
  import { graph } from './graphState.svelte';
  import { browserLayoutEngine, type LayoutResult } from './layout';
  import PhaseNode from './PhaseNode.svelte';
  import RunNode from './RunNode.svelte';
  import SessionNode from './SessionNode.svelte';
  import { sharedLinks } from './sharedLinks';
  import SharedFileEdge from './SharedFileEdge.svelte';
  import SharedFilesPopover from './SharedFilesPopover.svelte';
  import SpawnEdge from './SpawnEdge.svelte';

  let { sessionId }: { sessionId: string } = $props();

  const nodeTypes = { session: SessionNode, agent: AgentNode, run: RunNode, phase: PhaseNode };
  const edgeTypes = { spawn: SpawnEdge, shared: SharedFileEdge };
  const flow = useSvelteFlow();
  const engine = browserLayoutEngine();
  onDestroy(() => engine.dispose());

  // ---------- options ----------
  let detail = $state<DetailSetting>('auto');
  let sharedOn = $state(false);
  let sharedMode = $state<'edits' | 'all'>('edits');
  let collapsed = $state<Record<string, boolean>>({});
  let detailZoom = $state(1);
  let zoom = $state(1);
  let userMoved = false;
  let showFitPill = $state(false);
  let pillTimer: ReturnType<typeof setTimeout> | null = null;

  const session = $derived(world.session(sessionId));
  const replay = $derived(world.mode === 'replay');
  // Quantised so the structure is not rebuilt every frame while a replay plays.
  const viewTime = $derived(Math.floor(clock.viewTime / (replay ? 1000 : 60_000)) * (replay ? 1000 : 60_000));

  const model = $derived(
    session
      ? buildGraph(session, { detail, zoom: zoomBand(detailZoom), collapsed, viewTime, hideFuture: replay })
      : null,
  );

  graph.collapsedToggle = (runId: string) => {
    const node = model?.nodes.find((n) => n.runId === runId && n.kind === 'run');
    collapsed = { ...collapsed, [runId]: !(node?.collapsed ?? false) };
  };

  // ---------- layout ----------
  let nodes = $state.raw<Node[]>([]);
  let structEdges = $state.raw<Edge[]>([]);
  let applied: { model: GraphModel; layout: LayoutResult } | null = null;
  let appliedKey = '';
  let layoutTimer: ReturnType<typeof setTimeout> | null = null;
  let firstFit = true;

  function toFlow(m: GraphModel, lay: LayoutResult): Node[] {
    return m.nodes.map((n) => {
      const pos = n.parent ? { x: n.x, y: n.y } : (lay.positions[n.id] ?? { x: 0, y: 0 });
      const data =
        n.kind === 'session'
          ? { sessionId }
          : n.kind === 'agent'
            ? { sessionId, agentId: n.agentId!, detail: n.detail! }
            : n.kind === 'run'
              ? { sessionId, runId: n.runId!, collapsed: !!n.collapsed }
              : { sessionId, runId: n.runId!, index: n.phaseIndex! };
      return {
        id: n.id,
        type: n.kind,
        position: pos,
        parentId: n.parent ?? undefined,
        width: n.width,
        height: n.height,
        data,
        draggable: false,
        connectable: false,
        selectable: false,
        focusable: false,
        class: n.kind === 'run' || n.kind === 'phase' ? 'as-container' : undefined,
        zIndex: n.kind === 'run' ? 0 : n.kind === 'phase' ? 1 : 2,
      } satisfies Node;
    });
  }

  function toEdges(m: GraphModel): Edge[] {
    return m.edges.map((e) => ({
      id: e.id,
      source: e.source,
      target: e.target,
      type: 'spawn',
      data: { kind: e.kind, sessionId, agentId: e.agentId, runId: e.runId },
      selectable: false,
      focusable: false,
      zIndex: e.kind === 'sequence' ? 1 : 0,
    }));
  }

  $effect(() => {
    const m = model;
    if (!m || m.key === appliedKey) return;
    if (layoutTimer) clearTimeout(layoutTimer);
    const run = async () => {
      layoutTimer = null;
      const res = await engine.request(m.layout);
      if (!res) return;
      untrack(() => apply(m, res));
    };
    layoutTimer = setTimeout(run, applied ? 300 : 0);
    return () => {
      if (layoutTimer) clearTimeout(layoutTimer);
    };
  });

  let autoBandChanges = 0;
  let idsKey = '';

  function apply(m: GraphModel, res: LayoutResult) {
    const ids = m.nodes.map((n) => n.id).join(',');
    if (ids !== idsKey) {
      idsKey = ids;
      autoBandChanges = 0;
    }
    const grew = applied != null && m.agentCount > applied.model.agentCount;
    applied = { model: m, layout: res };
    appliedKey = m.key;
    nodes = toFlow(m, res);
    structEdges = toEdges(m);
    updateChain();
    if (firstFit || !userMoved) {
      firstFit = false;
      setTimeout(() => fit(false), 20);
    } else if (grew) {
      showFitPill = true;
      if (pillTimer) clearTimeout(pillTimer);
      pillTimer = setTimeout(() => (showFitPill = false), 6000);
    }
    consumeFocus();
  }

  /** The zoom a fit gives, from the layout bounds and the view size (no need to wait for it). */
  function fitZoom(maxZoom: number): number {
    const el = root?.getBoundingClientRect();
    if (!applied || !el || !applied.layout.width) return zoom;
    const z = Math.min((el.width - 96) / applied.layout.width, (el.height - 96) / applied.layout.height);
    return Math.max(0.15, Math.min(maxZoom, z));
  }

  async function fit(animate = true) {
    userMoved = false;
    showFitPill = false;
    const one = (applied?.model.agentCount ?? 0) <= 1;
    const maxZoom = one ? 1 : 2;
    const z = fitZoom(maxZoom);
    // An automatic fit may change the level of detail. If the new level's layout does not fit
    // at that level either (full is too big, dots too small), settle on compact; never more
    // than three changes per set of nodes, so layouts cannot ping-pong.
    if (zoomBand(z) !== zoomBand(detailZoom) && autoBandChanges < 3) {
      autoBandChanges += 1;
      detailZoom = autoBandChanges === 3 ? 0.6 : z;
    }
    await flow.fitView({ padding: '48px', maxZoom, minZoom: 0.15, duration: animate && !settings.reducedMotion ? 320 : 0 });
  }

  function onmovestart(e: MouseEvent | TouchEvent | null) {
    if (e) userMoved = true;
  }
  function onmove(_e: MouseEvent | TouchEvent | null, vp: Viewport) {
    zoom = vp.zoom;
    graph.zoom = vp.zoom;
  }
  function onmoveend(e: MouseEvent | TouchEvent | null, vp: Viewport) {
    if (e || userMoved) detailZoom = vp.zoom;
  }
  function zoomBy(dir: 1 | -1 | 0) {
    userMoved = true;
    const d = settings.reducedMotion ? 0 : 200;
    if (dir === 0) void flow.setZoom(1, { duration: d }).then(() => (detailZoom = 1));
    else void (dir > 0 ? flow.zoomIn({ duration: d }) : flow.zoomOut({ duration: d })).then(() => (detailZoom = flow.getZoom()));
  }

  // ---------- live view state for nodes ----------
  $effect(() => {
    graph.sessionId = sessionId;
  });
  $effect(() => {
    const s = session;
    const next = s ? loopBudget(Object.values(s.agents).filter((a) => a.agentId !== MAIN_AGENT_ID)) : new Set<string>();
    const cur = untrack(() => graph.loops);
    if (next.size !== cur.size || [...next].some((x) => !cur.has(x))) graph.loops = next;
  });

  function updateChain() {
    const c = selection.current;
    graph.chain = c?.kind === 'agent' && c.sessionId === sessionId && applied ? chainEdges(applied.model, c.id) : new Set();
  }
  $effect(() => {
    void selection.current;
    untrack(updateChain);
  });

  // Hover: the hovered agent, its parent and children stay bright; its edges brighten.
  $effect(() => {
    const h = hover.current;
    const s = untrack(() => session);
    if (!h || h.kind !== 'agent' || h.sessionId !== sessionId || !s || !applied) {
      graph.related = null;
      graph.hoverEdges = new Set();
      return;
    }
    const a = s.agents[h.id];
    const rel = new Set<string>([h.id]);
    if (a?.parentAgentId) rel.add(a.parentAgentId);
    for (const c of world.childrenOf(s).get(h.id) ?? []) rel.add(c.agentId);
    graph.related = rel;
    const nid = applied.model.agentNode.get(h.id);
    graph.hoverEdges = new Set(applied.model.edges.filter((e) => e.source === nid || e.target === nid).map((e) => e.id));
  });

  // A selected file rings its touchers.
  $effect(() => {
    const c = selection.current;
    const s = session;
    if (c?.kind !== 'file' || !s) {
      if (untrack(() => graph.fileTouch) != null) graph.fileTouch = null;
      return;
    }
    const f = s.files[c.id];
    graph.fileTouch = f ? new Map<string, FileTouch>(Object.entries(f.touches)) : new Map();
  });

  // ---------- shared-file links ----------
  let sharedEdges = $state.raw<Edge[]>([]);
  let sharedKey = '';
  let linkNote = $state<string | undefined>(undefined);
  $effect(() => {
    const s = session;
    const c = selection.current;
    if (!sharedOn || !s || !applied || c?.kind === 'file') {
      if (sharedKey) {
        sharedKey = '';
        sharedEdges = [];
      }
      linkNote = undefined;
      return;
    }
    void nodes;
    const res = sharedLinks(world.sharedPairs(s, sharedMode), applied.model.agentNode, c?.kind === 'agent' ? c.id : null, 40);
    linkNote = res.total > res.links.length && c?.kind !== 'agent' ? `Showing ${res.links.length} of ${res.total} links` : undefined;
    const key = res.links.map((l) => `${l.source}>${l.target}:${l.files.length}:${l.bothEdited}`).join(';');
    if (key === sharedKey) return;
    sharedKey = key;
    sharedEdges = res.links.map((l) => ({
      id: `shared:${l.source}>${l.target}`,
      source: l.source,
      target: l.target,
      type: 'shared',
      data: { files: l.files, bothEdited: l.bothEdited },
      selectable: false,
      focusable: false,
      zIndex: 3,
    }));
  });
  const edges = $derived(sharedEdges.length ? [...structEdges, ...sharedEdges] : structEdges);

  // ---------- focus requests (double-click in Workflow) ----------
  function absBox(id: string): { x: number; y: number; w: number; h: number } | null {
    if (!applied) return null;
    const byId = new Map(applied.model.nodes.map((n) => [n.id, n]));
    let n = byId.get(id);
    if (!n) return null;
    const w = n.width;
    const h = n.height;
    let x = 0;
    let y = 0;
    while (n) {
      if (n.parent) {
        x += n.x;
        y += n.y;
        n = byId.get(n.parent);
      } else {
        const p = applied.layout.positions[n.id] ?? { x: 0, y: 0 };
        x += p.x;
        y += p.y;
        break;
      }
    }
    return { x, y, w, h };
  }

  function consumeFocus() {
    const req = links.graphFocus;
    if (!req || req.sessionId !== sessionId || !applied) return;
    const nid = applied.model.agentNode.get(req.agentId);
    const box = nid && absBox(nid);
    if (!box) return;
    links.graphFocus = null;
    userMoved = true;
    void flow.setCenter(box.x + box.w / 2, box.y + box.h / 2, { zoom: Math.max(flow.getZoom(), 0.9), duration: settings.reducedMotion ? 0 : 320 });
  }
  $effect(() => {
    void links.graphFocus;
    untrack(consumeFocus);
  });

  // ---------- keyboard ----------
  function agentBoxes(): { id: string; x: number; y: number }[] {
    if (!applied) return [];
    const out: { id: string; x: number; y: number }[] = [];
    for (const n of applied.model.nodes) {
      if (n.kind !== 'agent' && n.kind !== 'session') continue;
      const b = absBox(n.id);
      if (b) out.push({ id: n.agentId!, x: b.x + b.w / 2, y: b.y + b.h / 2 });
    }
    return out;
  }

  function moveSelection(dx: number, dy: number) {
    const boxes = agentBoxes();
    const c = selection.current;
    const curId = c?.kind === 'agent' && c.sessionId === sessionId ? c.id : c?.kind === 'session' ? MAIN_AGENT_ID : null;
    const cur = boxes.find((b) => b.id === curId);
    if (!cur) {
      const first = boxes.find((b) => b.id !== MAIN_AGENT_ID) ?? boxes[0];
      if (first) select(first.id);
      return;
    }
    let best: { id: string; score: number } | null = null;
    for (const b of boxes) {
      if (b.id === cur.id) continue;
      const vx = b.x - cur.x;
      const vy = b.y - cur.y;
      const along = vx * dx + vy * dy;
      if (along <= 1) continue;
      const across = Math.abs(vx * dy - vy * dx);
      const score = along + across * 2;
      if (!best || score < best.score) best = { id: b.id, score };
    }
    if (best) select(best.id);
  }

  function select(agentId: string) {
    if (agentId === MAIN_AGENT_ID) actions.select(sel.session(sessionId));
    else actions.select(sel.agent(sessionId, agentId));
    const nid = applied?.model.agentNode.get(agentId);
    const box = nid && absBox(nid);
    if (box) {
      const vp = flow.getViewport();
      const el = root?.getBoundingClientRect();
      if (el) {
        const sx = box.x * vp.zoom + vp.x;
        const sy = box.y * vp.zoom + vp.y;
        if (sx < 0 || sy < 0 || sx + box.w * vp.zoom > el.width || sy + box.h * vp.zoom > el.height) {
          userMoved = true;
          void flow.setCenter(box.x + box.w / 2, box.y + box.h / 2, { zoom: vp.zoom, duration: settings.reducedMotion ? 0 : 200 });
        }
      }
    }
  }

  // Runs in the capture phase, before the frame's global handler, which then skips the keys
  // this view handled (preventDefault).
  function onkey(e: KeyboardEvent) {
    if (e.defaultPrevented || e.metaKey || e.ctrlKey || e.altKey || ui.anyDialog) return;
    const t = e.target as HTMLElement;
    if (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA') return;
    const k = e.key;
    const handled = () => e.preventDefault();
    if (k === 'f' || k === 'F') return handled(), void fit();
    if (k === '+' || k === '=') return handled(), zoomBy(1);
    if (k === '-' || k === '_') return handled(), zoomBy(-1);
    if (k === '0') return handled(), zoomBy(0);
    if (k === 's' || k === 'S') return handled(), (sharedOn = !sharedOn);
    // Arrow keys move the selection only while the graph has focus (replay uses them to seek).
    if (!root?.contains(document.activeElement)) return;
    if (k === 'ArrowRight') return handled(), moveSelection(1, 0);
    if (k === 'ArrowLeft') return handled(), moveSelection(-1, 0);
    if (k === 'ArrowDown') return handled(), moveSelection(0, 1);
    if (k === 'ArrowUp') return handled(), moveSelection(0, -1);
  }

  // ---------- tooltip ----------
  let tipShown = $state<{ agent: Agent; x: number; y: number } | null>(null);
  $effect(() => {
    const t = graph.tip;
    if (!t) {
      tipShown = null;
      return;
    }
    const timer = setTimeout(() => {
      const a = world.agent(sessionId, t.agentId);
      if (a) tipShown = { agent: a, x: t.x, y: t.y };
    }, 400);
    return () => clearTimeout(timer);
  });

  let root: HTMLDivElement | undefined = $state();
  const showMinimap = $derived(nodes.length >= 6);

  function miniClass(n: Node): string {
    if (n.type === 'agent') {
      const a = world.agent(sessionId, (n.data as { agentId: string }).agentId);
      return `mm st-${a ? AGENT_STATE[a.state].tone : 'idle'}`;
    }
    if (n.type === 'session') return 'mm mm-session';
    return 'mm mm-container';
  }
</script>

<svelte:window onkeydowncapture={onkey} />

<div class="graph canvas-surface" bind:this={root} tabindex="-1" role="application" aria-label="Agent graph. Arrow keys move the selection; F fits; S toggles shared files.">
  <svg class="defs" aria-hidden="true" width="0" height="0">
    <defs>
      <marker id="as-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
        <path d="M0 1 L9 5 L0 9 z" class="as-arrow-head" />
      </marker>
    </defs>
  </svg>

  <SvelteFlow
    {nodes}
    {edges}
    {nodeTypes}
    {edgeTypes}
    minZoom={0.15}
    maxZoom={2}
    nodesDraggable={false}
    nodesConnectable={false}
    elementsSelectable={false}
    zoomOnDoubleClick={false}
    panOnScroll={true}
    onlyRenderVisibleElements={true}
    proOptions={{ hideAttribution: true }}
    {onmovestart}
    {onmove}
    {onmoveend}
    onpaneclick={() => (graph.linkPopover = null)}
  >
    {#if showMinimap}
      <MiniMap width={160} height={110} nodeClass={miniClass} nodeBorderRadius={3} pannable zoomable ariaLabel="Graph overview" />
    {/if}
  </SvelteFlow>

  <GraphToolbar
    {zoom}
    {detail}
    sharedFiles={sharedOn}
    {sharedMode}
    {linkNote}
    onfit={() => fit()}
    onzoomin={() => zoomBy(1)}
    onzoomout={() => zoomBy(-1)}
    onzoomreset={() => zoomBy(0)}
    ondetail={(d) => (detail = d)}
    onshared={(on) => (sharedOn = on)}
    onsharedmode={(m) => (sharedMode = m)}
  />

  {#if showFitPill}
    <button type="button" class="pill t-small" onclick={() => fit()}>New agents · Fit</button>
  {/if}

  {#if session && Object.keys(session.agents).length <= 1 && Object.keys(session.runs).length === 0}
    <div class="hint">
      <p class="t-title">Just the session so far</p>
      <p class="t-small">This session has not started any agents.</p>
    </div>
  {/if}

  {#if tipShown}
    {@const a = tipShown.agent}
    <div class="tip t-small" style="left: {tipShown.x + 14}px; top: {tipShown.y + 14}px" role="tooltip">
      <p class="t-strong">{a.label}</p>
      <p>{AGENT_STATE[a.state].label}{a.startedAt != null ? ` · ${formatDuration(Math.max(0, (a.endedAt ?? clock.viewTime) - a.startedAt))}` : ''}</p>
      {#if a.task}<p class="task">{taskSummary(a.task, 280)}</p>{/if}
    </div>
  {/if}

  {#if graph.linkPopover}
    <SharedFilesPopover
      files={graph.linkPopover.files}
      x={graph.linkPopover.x}
      y={graph.linkPopover.y}
      cwd={session?.cwd ?? null}
      onclose={() => (graph.linkPopover = null)}
      onselect={(p) => {
        graph.linkPopover = null;
        actions.select(sel.file(sessionId, p));
      }}
    />
  {/if}
</div>

<style>
  .graph { position: relative; height: 100%; width: 100%; overflow: hidden; }
  .defs { position: absolute; }
  .graph :global(.svelte-flow) {
    background: transparent;
    --xy-minimap-background-color-default: var(--surface-raised);
    --xy-minimap-mask-background-color-default: color-mix(in srgb, var(--surface-sunken) 70%, transparent);
    --xy-minimap-mask-stroke-color-default: var(--border-strong);
    --xy-edge-label-background-color-default: transparent;
  }
  .graph :global(.svelte-flow__node) {
    transition: transform var(--dur-3) var(--ease-in-out);
    background: none;
    border: 0;
    padding: 0;
    box-shadow: none;
    border-radius: 0;
    color: inherit;
    font-size: inherit;
    text-align: left;
  }
  .graph :global(.as-container) { pointer-events: none; }
  .graph :global(.as-container .head),
  .graph :global(.as-container .head *),
  .graph :global(.as-container.svelte-flow__node-run .collapsed) { pointer-events: all; }
  .graph :global(.as-handle) {
    opacity: 0;
    width: 1px;
    height: 1px;
    min-width: 0;
    min-height: 0;
    border: 0;
    pointer-events: none;
  }
  .graph :global(.as-edge) { stroke: var(--border-strong); stroke-width: var(--edge-width); transition: stroke var(--dur-2) var(--ease-out); }
  .graph :global(.as-edge.run) { stroke-width: 2px; }
  .graph :global(.as-edge.sequence) { stroke: var(--border-default); stroke-width: 1px; }
  .graph :global(.as-edge.active) { stroke: var(--state-running); stroke-dasharray: 6 6; animation: as-dash var(--dur-loop) linear infinite; }
  .graph :global(.as-edge.hl) { stroke: var(--select-ring); stroke-width: 2px; }
  .graph :global(.as-arrow-head) { fill: var(--border-strong); }
  .graph :global(.as-shared) { stroke: var(--text-muted); stroke-opacity: 0.6; stroke-dasharray: 6 4; cursor: pointer; }
  .graph :global(.as-shared.edits) { stroke: var(--accent-text); stroke-opacity: 0.7; }
  .graph :global(.svelte-flow__edge-interaction) { cursor: pointer; }
  .graph :global(.as-shared-label) {
    min-width: 20px;
    height: 18px;
    padding: 0 5px;
    border-radius: var(--radius-pill);
    background: var(--surface-raised);
    border: var(--border-width) solid var(--border-default);
    color: var(--accent-text);
    font-size: var(--text-xs);
  }
  .graph :global(.svelte-flow__minimap) {
    border: var(--border-width) solid var(--border-default);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-2);
    overflow: hidden;
  }
  .graph :global(.svelte-flow__minimap .mm) { fill: var(--st, var(--border-strong)); }
  .graph :global(.svelte-flow__minimap .mm-container) { fill: var(--surface-sunken); stroke: var(--border-default); }
  .graph :global(.svelte-flow__minimap .mm-session) { fill: var(--text-secondary); }
  @keyframes -global-as-dash { to { stroke-dashoffset: -12; } }

  .pill {
    position: absolute;
    top: var(--space-3);
    left: 50%;
    transform: translateX(-50%);
    z-index: var(--z-sticky);
    padding: 4px var(--space-3);
    border-radius: var(--radius-pill);
    background: var(--accent);
    color: var(--text-on-accent);
    box-shadow: var(--shadow-2);
    animation: as-slide-in var(--dur-2) var(--ease-out);
  }
  .hint {
    position: absolute;
    left: 50%;
    bottom: var(--space-8);
    transform: translateX(-50%);
    text-align: center;
    color: var(--text-secondary);
    pointer-events: none;
  }
  .tip {
    position: fixed;
    z-index: var(--z-popover);
    max-width: 320px;
    padding: var(--space-2) var(--space-3);
    background: var(--surface-raised);
    border: var(--border-width) solid var(--border-default);
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow-3);
    color: var(--text-primary);
    pointer-events: none;
  }
  .tip .task { color: var(--text-secondary); margin-top: 2px; }
</style>
