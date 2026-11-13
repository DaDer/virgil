<script lang="ts">
  // ux-spec 2.3: All sessions › <session> › <run>. Each crumb opens a menu.
  import type { Session, WorkflowRun } from '../contract';
  import Icon from '../components/Icon.svelte';
  import Menu from '../components/Menu.svelte';
  import MenuItem from '../components/MenuItem.svelte';
  import StateIcon from '../components/StateIcon.svelte';
  import { sessionTitle } from '../data/derive';
  import { formatClock, formatRelative, projectName } from '../format';
  import { RUN_STATUS, SESSION_STATE, sessionState } from '../states';
  import { clock } from '../stores/clock.svelte';
  import type { Scope } from '../stores/route.svelte';
  import { lastActivity } from '../data/derive';

  let {
    sessions,
    runs,
    scope,
    showSession = true,
    onoverview,
    onsession,
    onrun,
  }: {
    sessions: Session[];
    runs: WorkflowRun[];
    scope: Scope;
    showSession?: boolean;
    onoverview: () => void;
    onsession: (id: string) => void;
    onrun: (id: string) => void;
  } = $props();

  let sessionBtn: HTMLButtonElement | undefined = $state();
  let runBtn: HTMLButtonElement | undefined = $state();
  let sessionMenu = $state(false);
  let runMenu = $state(false);

  const current = $derived(sessions.find((s) => s.sessionId === scope.sessionId) ?? null);
  const sortedRuns = $derived([...runs].sort((a, b) => (b.startedAt ?? 0) - (a.startedAt ?? 0)));
  const currentRun = $derived(sortedRuns.find((r) => r.runId === scope.runId) ?? sortedRuns.find((r) => r.status === 'running') ?? sortedRuns[0] ?? null);
</script>

<nav class="crumbs" aria-label="Scope">
  <button type="button" class="crumb root" onclick={onoverview}>All sessions</button>
  {#if showSession && current}
    <Icon name="chevron-right" size={14} />
    <button type="button" class="crumb" bind:this={sessionBtn} aria-haspopup="menu" aria-expanded={sessionMenu} onclick={() => (sessionMenu = !sessionMenu)}>
      <StateIcon state={SESSION_STATE[sessionState(current)]} size={14} />
      <span class="ellipsis">{sessionTitle(current)}</span>
      <Icon name="chevron-down" size={14} />
    </button>
    {#if sortedRuns.length && currentRun}
      <Icon name="chevron-right" size={14} />
      <button type="button" class="crumb" bind:this={runBtn} aria-haspopup="menu" aria-expanded={runMenu} onclick={() => (runMenu = !runMenu)}>
        <StateIcon state={RUN_STATUS[currentRun.status]} size={14} />
        <span class="ellipsis">{currentRun.name}</span>
        <Icon name="chevron-down" size={14} />
      </button>
    {/if}
  {/if}
</nav>

<Menu anchor={sessionBtn} open={sessionMenu} onclose={() => (sessionMenu = false)} label="Sessions" width={340}>
  {#each sessions as s (s.sessionId)}
    <MenuItem checked={s.sessionId === scope.sessionId} onselect={() => { sessionMenu = false; onsession(s.sessionId); }}>
      <StateIcon state={SESSION_STATE[sessionState(s)]} size={14} />
      <span class="mi">
        <span class="ellipsis">{sessionTitle(s)}</span>
        <span class="sub t-small ellipsis">{projectName(s.cwd)} · {formatRelative(lastActivity(s), clock.viewTime)}</span>
      </span>
    </MenuItem>
  {/each}
</Menu>

<Menu anchor={runBtn} open={runMenu} onclose={() => (runMenu = false)} label="Runs" width={320}>
  {#each sortedRuns as r (r.runId)}
    <MenuItem checked={r.runId === currentRun?.runId} onselect={() => { runMenu = false; onrun(r.runId); }}>
      <StateIcon state={RUN_STATUS[r.status]} size={14} />
      <span class="mi">
        <span class="ellipsis">{r.name}</span>
        <span class="sub t-small">{formatClock(r.startedAt, false)} · {r.declaredAgentCount ?? r.agentIds.length} agents</span>
      </span>
    </MenuItem>
  {/each}
</Menu>

<style>
  .crumbs { display: flex; align-items: center; gap: var(--space-1); min-width: 0; color: var(--text-muted); }
  .crumb {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    max-width: 260px;
    min-width: 0;
    height: var(--size-control);
    padding: 0 var(--space-2);
    border-radius: var(--radius-sm);
    color: var(--text-primary);
    font-weight: var(--weight-medium);
  }
  .crumb:hover { background: var(--surface-hover); }
  .root { color: var(--text-secondary); }
  .mi { display: flex; flex-direction: column; min-width: 0; }
  .sub { color: var(--text-muted); }
</style>
