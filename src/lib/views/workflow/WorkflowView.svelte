<script lang="ts">
  // ux-spec 3.3: progress of one run, phase by phase. Props: none (reads stores).
  // While a run is in flight its phases, states and totals come from the reconstruction in the
  // snapshot (data-contract 4); "total" in a phase means agents seen so far.
  import type { Agent } from '../../contract';
  import EmptyState from '../../components/EmptyState.svelte';
  import { activePhaseIndex, runTotals } from '../../data/derive';
  import { actions } from '../../stores/actions';
  import { clock } from '../../stores/clock.svelte';
  import { route, scope } from '../../stores/route.svelte';
  import { sel, selection } from '../../stores/selection.svelte';
  import { ui } from '../../stores/ui.svelte';
  import { world } from '../../stores/world.svelte';
  import { loopBudget, startsAfter } from '../shared/motion';
  import LogPanel from './LogPanel.svelte';
  import { activityLines, columns, moveInBoard, runBar } from './model';
  import PhaseColumn from './PhaseColumn.svelte';
  import RunHeader from './RunHeader.svelte';

  const session = $derived(world.session(scope.sessionId));
  const run = $derived(world.runOrCurrent(scope.sessionId, scope.runId));
  const runs = $derived(session ? Object.values(session.runs).sort((a, b) => (b.startedAt ?? 0) - (a.startedAt ?? 0)) : []);
  const replay = $derived(world.mode === 'replay');
  const now = $derived(clock.viewTime);

  const byPhase = $derived.by(() => {
    if (!session || !run) return new Map<number, Agent[]>();
    const m = world.agentsByPhase(session, run);
    if (!replay) return m;
    const out = new Map<number, Agent[]>();
    for (const [k, list] of m) out.set(k, list.filter((a) => !startsAfter(a, now)));
    return out;
  });
  const active = $derived(session && run ? activePhaseIndex(session, run) : null);
  const cols = $derived(session && run ? columns(session, run, byPhase, active) : []);
  const totals = $derived(session && run ? runTotals(session, run) : null);
  const bar = $derived(runBar(cols));
  const loops = $derived(session && run ? loopBudget(run.agentIds.map((id) => session.agents[id]).filter(Boolean)) : new Set<string>());
  const lines = $derived(session && run ? activityLines(session, run, now) : []);
  const noAgents = $derived(cols.every((c) => c.agents.length === 0));

  let logOpen = $state(false);

  function onkey(e: KeyboardEvent) {
    if (e.defaultPrevented || e.metaKey || e.ctrlKey || e.altKey || ui.anyDialog || !session) return;
    if (!['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(e.key)) return;
    if (!board?.contains(document.activeElement)) return;
    e.preventDefault();
    const c = selection.current;
    const cur = c?.kind === 'agent' && c.sessionId === session.sessionId ? c.id : null;
    const next = moveInBoard(cols, cur, e.key as 'ArrowUp');
    if (next && next !== cur) {
      actions.select(sel.agent(session.sessionId, next));
      queueMicrotask(() => board?.querySelector<HTMLElement>(`[data-agent="${CSS.escape(next)}"] [role="button"]`)?.focus());
    }
  }
  let board: HTMLDivElement | undefined = $state();
</script>

<svelte:window onkeydowncapture={onkey} />

<section class="view" aria-label="Workflow">
  {#if !session}
    <EmptyState icon="workflow" title="No session in scope" text="Pick a session on the Overview first." />
  {:else if !run || !totals}
    <EmptyState icon="workflow" title="No workflow runs" text="This session has not run a workflow." action={{ label: 'Show graph', icon: 'graph', onclick: () => route.navigate('graph') }} />
  {:else}
    <RunHeader
      {run}
      {runs}
      {totals}
      {bar}
      phaseOf={{ current: active, count: cols.length }}
      selected={selection.is('run', run.runId, session.sessionId)}
      onselect={() => actions.select(sel.run(session.sessionId, run.runId))}
      onpick={(id) => scope.openRun(id)}
    />
    <div class="board" bind:this={board}>
      {#if noAgents}
        <p class="waiting t-small">Waiting for the first agent</p>
      {/if}
      <div class="cols" class:few={cols.length <= 3}>
        {#each cols as col (col.phase.index)}
          <div class="colwrap" class:stretch={cols.length <= 3}>
            <PhaseColumn {col} sessionId={session.sessionId} runId={run.runId} {loops} />
          </div>
        {:else}
          <p class="waiting t-small">This run has not declared any phases yet.</p>
        {/each}
      </div>
    </div>
    <LogPanel
      {lines}
      open={logOpen}
      ontoggle={() => (logOpen = !logOpen)}
      note={run.final ? 'The run file’s own log lines and result are not part of the data contract yet.' : 'Log lines appear when the run finishes; until then this shows agent starts and ends.'}
    />
  {/if}
</section>

<style>
  .view { height: 100%; display: flex; flex-direction: column; min-height: 0; background: var(--surface-app); }
  .board { position: relative; flex: 1; min-height: 0; display: flex; flex-direction: column; overflow: hidden; }
  .cols { flex: 1; min-height: 0; display: flex; gap: var(--space-3); padding: var(--space-4) var(--space-6); overflow-x: auto; overflow-y: hidden; }
  .colwrap { display: flex; min-height: 0; }
  .colwrap.stretch { flex: 1 1 288px; max-width: 420px; }
  .colwrap.stretch > :global(.col) { flex: 1; }
  .waiting { padding: var(--space-3) var(--space-6) 0; color: var(--text-muted); }
</style>
