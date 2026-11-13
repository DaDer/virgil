<script lang="ts">
  // ux-spec 3.3 RunHeader: state, name, timer, run picker, summary, whole-run bar, totals.
  import type { WorkflowRun } from '../../contract';
  import Menu from '../../components/Menu.svelte';
  import MenuItem from '../../components/MenuItem.svelte';
  import StateIcon from '../../components/StateIcon.svelte';
  import StatTile from '../../components/StatTile.svelte';
  import Icon from '../../components/Icon.svelte';
  import type { RunTotals } from '../../data/derive';
  import { formatCount, formatDate, formatDuration, formatInt } from '../../format';
  import { RUN_STATUS } from '../../states';
  import { clock } from '../../stores/clock.svelte';
  import type { RunBarSegment } from './model';

  let {
    run,
    runs,
    totals,
    bar,
    phaseOf,
    selected,
    onselect,
    onpick,
  }: {
    run: WorkflowRun;
    runs: WorkflowRun[];
    totals: RunTotals;
    bar: RunBarSegment[];
    phaseOf: { current: number | null; count: number };
    selected: boolean;
    onselect: () => void;
    onpick: (runId: string) => void;
  } = $props();

  const st = $derived(RUN_STATUS[run.status]);
  const running = $derived(run.status === 'running');
  const elapsed = $derived(run.startedAt != null ? Math.max(0, (run.endedAt ?? clock.viewTime) - run.startedAt) : null);
  let pickBtn: HTMLButtonElement | undefined = $state();
  let pickOpen = $state(false);
  let summaryOpen = $state(false);
</script>

<header class="head">
  <div class="line1">
    <button type="button" class="name" class:selected onclick={onselect} title="Select the run">
      <StateIcon state={st} size={18} />
      <span class="t-display ellipsis">{run.name}</span>
    </button>
    <span class="state t-small st-{st.tone}">{st.label}</span>
    <span class="timer num" title={running ? 'Elapsed' : 'Duration'}>{formatDuration(elapsed)}</span>
    <span class="flex"></span>
    {#if runs.length > 1}
      <button type="button" class="picker t-small" bind:this={pickBtn} aria-haspopup="menu" aria-expanded={pickOpen} onclick={() => (pickOpen = !pickOpen)}>
        {runs.length} runs <Icon name="chevron-down" size={14} />
      </button>
      <Menu anchor={pickBtn} open={pickOpen} onclose={() => (pickOpen = false)} label="Workflow runs" placement="bottom-end" width={320}>
        {#each runs as r (r.runId)}
          <MenuItem checked={r.runId === run.runId} hint="{r.agentIds.length} agents" onselect={() => ((pickOpen = false), onpick(r.runId))}>
            {r.name} · {r.startedAt != null ? formatDate(r.startedAt) : '–'}
          </MenuItem>
        {/each}
      </Menu>
    {/if}
  </div>
  {#if run.summary}
    <button type="button" class="summary t-small" class:open={summaryOpen} onclick={() => (summaryOpen = !summaryOpen)} title={summaryOpen ? 'Show less' : 'Show all'}>{run.summary}</button>
  {/if}
  <div class="barline">
    <div class="runbar" role="img" aria-label="Run progress by phase">
      {#each bar as seg (seg.index)}
        <span class="seg" title="{seg.index} {seg.title}: {Math.round(seg.done * 100)}% done">
          <span class="fill st-done" style="width: {seg.done * 100}%"></span>
          <span class="fill st-running" style="width: {seg.running * 100}%">{#if running && seg.running > 0}<span class="sheen loops"></span>{/if}</span>
          <span class="fill st-error" style="width: {seg.failed * 100}%"></span>
        </span>
      {/each}
    </div>
    <span class="phaseof t-small num">{phaseOf.current != null ? `Phase ${phaseOf.current} of ${phaseOf.count}` : `${phaseOf.count} phases`}</span>
  </div>
  <div class="tiles">
    <StatTile label="Agents" value={formatInt(totals.agents)} />
    <StatTile label="Done" value={formatInt(totals.done)} tone="done" />
    <StatTile label="Running" value={formatInt(totals.running)} tone="running" />
    <StatTile label="Waiting" value={formatInt(totals.waiting)} tone="queued" />
    {#if totals.failed > 0}<StatTile label="Failed" value={formatInt(totals.failed)} tone="error" />{/if}
    {#if totals.stopped > 0}<StatTile label="Stopped" value={formatInt(totals.stopped)} tone="stopped" />{/if}
    <StatTile label="Tokens" value={formatCount(totals.tokens)} sub={run.final ? undefined : 'sum so far'} />
    <StatTile label="Tool calls" value={formatInt(totals.toolCalls)} />
    {#if totals.durationMs != null}<StatTile label="Duration" value={formatDuration(totals.durationMs)} />{/if}
  </div>
</header>

<style>
  .head { display: flex; flex-direction: column; gap: var(--space-2); padding: var(--space-4) var(--space-6) var(--space-3); border-bottom: var(--border-width) solid var(--border-subtle); background: var(--surface-panel); }
  .line1 { display: flex; align-items: center; gap: var(--space-3); min-width: 0; }
  .name { display: inline-flex; align-items: center; gap: var(--space-2); min-width: 0; padding: 2px 6px; margin-left: -6px; border-radius: var(--radius-sm); color: var(--text-primary); }
  .name:hover { background: var(--surface-hover); }
  .name.selected { outline: 2px solid var(--select-ring); outline-offset: 2px; }
  .state { padding: 1px 8px; border-radius: var(--radius-pill); background: var(--st-soft); color: var(--st-text); font-weight: var(--weight-semibold); }
  .timer { color: var(--text-secondary); }
  .flex { flex: 1; }
  .picker { display: inline-flex; align-items: center; gap: 4px; height: var(--size-control); padding: 0 var(--space-2); border-radius: var(--radius-sm); border: var(--border-width) solid var(--border-default); color: var(--text-primary); }
  .picker:hover { background: var(--surface-hover); }
  .summary { text-align: left; color: var(--text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 100%; }
  .summary.open { white-space: normal; }
  .barline { display: flex; align-items: center; gap: var(--space-3); }
  .runbar { flex: 1; display: flex; gap: 3px; height: 8px; }
  .seg { position: relative; flex: 1; display: flex; overflow: hidden; border-radius: var(--radius-pill); background: var(--surface-sunken); }
  .fill { position: relative; overflow: hidden; background: var(--st); transition: width var(--dur-2) var(--ease-out); }
  .sheen { position: absolute; inset: 0; background: linear-gradient(90deg, transparent, color-mix(in srgb, var(--surface-card) 55%, transparent), transparent); animation: as-sheen calc(var(--dur-loop) * 1.5) linear infinite; }
  .phaseof { color: var(--text-secondary); white-space: nowrap; }
  .tiles { display: flex; flex-wrap: wrap; gap: var(--space-2); }
</style>
