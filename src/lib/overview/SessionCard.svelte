<script lang="ts">
  // ux-spec 3.1 SessionCard.
  import type { Session } from '../contract';
  import Chip from '../components/Chip.svelte';
  import Menu from '../components/Menu.svelte';
  import MenuItem from '../components/MenuItem.svelte';
  import Sparkline from '../components/Sparkline.svelte';
  import StateIcon from '../components/StateIcon.svelte';
  import { copyText } from '../components/clipboard';
  import { lastActivity, sessionTitle } from '../data/derive';
  import { formatCount, formatDuration, formatRelative, projectName } from '../format';
  import { SESSION_STATE, sessionState, type SessionState } from '../states';
  import { clock } from '../stores/clock.svelte';
  import type { Route } from '../stores/route.svelte';

  let {
    session,
    stats,
    activity,
    now: nowLine,
    selected,
    dimmed = false,
    onselect,
    onopen,
  }: {
    session: Session;
    stats: { running: number; agents: number; runs: number; tokens: number | null };
    activity: number[];
    now?: { phaseTitle: string; done: number; total: number };
    selected: boolean;
    dimmed?: boolean;
    onselect: () => void;
    onopen: (to: Route) => void;
  } = $props();

  const sstate: SessionState = $derived(sessionState(session));
  const meta = $derived(SESSION_STATE[sstate]);
  const live = $derived(sstate === 'busy' || sstate === 'waiting' || sstate === 'idle');
  const age = $derived(
    live
      ? session.startedAt != null
        ? formatDuration(Math.max(0, clock.viewTime - session.startedAt))
        : ''
      : `ended ${formatRelative(lastActivity(session), clock.viewTime)}`,
  );

  let card: HTMLElement | undefined = $state();
  let menuOpen = $state(false);
  let copied = $state(false);

  function key(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      onopen('graph');
    } else if (e.key === ' ') {
      e.preventDefault();
      onselect();
    } else if (e.key === 'ContextMenu' || (e.shiftKey && e.key === 'F10')) {
      e.preventDefault();
      menuOpen = true;
    }
  }
</script>

<div
  role="button"
  bind:this={card}
  class="card st-{meta.tone}"
  class:busy={sstate === 'busy'}
  class:selected
  class:dimmed
  tabindex="0"
  aria-label="{sessionTitle(session)}, {meta.label}"
  aria-current={selected ? 'true' : undefined}
  onclick={onselect}
  ondblclick={() => onopen('graph')}
  onkeydown={key}
  oncontextmenu={(e) => {
    e.preventDefault();
    onselect();
    menuOpen = true;
  }}
>
  <span class="edge" aria-hidden="true"></span>
  <div class="row top">
    <span class="state">
      <StateIcon state={meta} size={16} animated={sstate === 'busy'} />
      <span class="t-label state-label">{meta.label}</span>
    </span>
    <span class="age t-small num">{age}</span>
  </div>
  <h3 class="t-title title" title={session.title ?? undefined}>{sessionTitle(session)}</h3>
  <p class="project t-mono" title={session.cwd ?? undefined}>{projectName(session.cwd)}</p>
  <div class="spark">
    <Sparkline values={activity} height={28} highlightLast={sstate === 'busy' ? 2 : 0} label="Tool calls per minute" />
  </div>
  <div class="row chips">
    {#if stats.running > 0}
      <Chip tone="running" label="{stats.running} running">
        {#snippet leading()}<StateIcon state="running" size={12} />{/snippet}
      </Chip>
    {/if}
    <Chip label="{stats.agents} {stats.agents === 1 ? 'agent' : 'agents'}" />
    {#if stats.runs > 0}<Chip label="{stats.runs} {stats.runs === 1 ? 'run' : 'runs'}" />{/if}
    <span class="tokens num t-small" title="Tokens over all agents">{stats.tokens == null ? '–' : `${formatCount(stats.tokens)} tok`}</span>
  </div>
  {#if nowLine}
    <p class="now t-small"><span class="muted">Now:</span> {nowLine.phaseTitle} · {nowLine.done} of {nowLine.total} agents done</p>
  {/if}
</div>

<Menu anchor={card} open={menuOpen} onclose={() => (menuOpen = false)} label="Session actions" placement="bottom-end">
  <MenuItem icon="graph" onselect={() => { menuOpen = false; onopen('graph'); }}>Open graph</MenuItem>
  <MenuItem icon="workflow" onselect={() => { menuOpen = false; onopen('workflow'); }}>Open workflow</MenuItem>
  <MenuItem icon="timeline" onselect={() => { menuOpen = false; onopen('timeline'); }}>Open timeline</MenuItem>
  <MenuItem
    icon="copy"
    onselect={async () => {
      copied = await copyText(session.sessionId);
      setTimeout(() => { menuOpen = false; copied = false; }, 500);
    }}
  >{copied ? 'Copied' : 'Copy session id'}</MenuItem>
</Menu>

<style>
  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    min-height: 168px;
    padding: var(--pad-4) var(--pad-4) var(--pad-3) calc(var(--pad-4) + 3px);
    border-radius: var(--radius-lg);
    background: var(--surface-card);
    border: var(--border-width) solid var(--border-default);
    box-shadow: var(--shadow-1);
    cursor: pointer;
    overflow: hidden;
    content-visibility: auto;
    contain-intrinsic-size: 320px 168px;
    transition:
      transform var(--dur-1) var(--ease-out),
      box-shadow var(--dur-1) var(--ease-out),
      border-color var(--dur-2) var(--ease-out),
      opacity var(--dur-1) var(--ease-out);
    animation: as-appear var(--dur-2) var(--ease-out);
  }
  .card:hover { box-shadow: var(--shadow-2); transform: translateY(-1px); }
  :global(:root[data-theme='poster']) .card:hover { transform: translate(-1px, -1px); }
  :global(:root[data-theme='poster']) .card:active { transform: translate(2px, 2px); box-shadow: var(--shadow-0); }
  .selected { outline: 2px solid var(--select-ring); outline-offset: 2px; }
  .dimmed { opacity: 0.25; }
  .edge { position: absolute; left: 0; top: 0; bottom: 0; width: 3px; background: var(--st); }
  .row { display: flex; align-items: center; gap: var(--space-2); }
  .top { justify-content: space-between; }
  .state { display: inline-flex; align-items: center; gap: 6px; }
  .state-label { color: var(--st-text); }
  .age { color: var(--text-muted); white-space: nowrap; }
  .title {
    margin-top: var(--space-1);
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    overflow: hidden;
    overflow-wrap: anywhere;
  }
  .project { color: var(--text-secondary); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .spark { margin: var(--space-2) 0 var(--space-2); }
  .chips { flex-wrap: wrap; margin-top: auto; }
  .tokens { margin-left: auto; color: var(--text-secondary); }
  .now { color: var(--text-primary); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
</style>
