<script lang="ts">
  // ux-spec 2.1: the window grid, the whole-app states (2.10), dialogs and global shortcuts.
  import type { Snippet } from 'svelte';
  import { MAIN_AGENT_ID } from '../contract';
  import EmptyState from '../components/EmptyState.svelte';
  import Icon from '../components/Icon.svelte';
  import IconSprite from '../components/IconSprite.svelte';
  import Skeleton from '../components/Skeleton.svelte';
  import StateBadge from '../components/StateBadge.svelte';
  import TypeGlyph from '../components/TypeGlyph.svelte';
  import { bucketize } from '../data/derive';
  import { actions } from '../stores/actions';
  import { clock } from '../stores/clock.svelte';
  import { route, scope, type Route } from '../stores/route.svelte';
  import { selection } from '../stores/selection.svelte';
  import { settings } from '../stores/settings.svelte';
  import { ui } from '../stores/ui.svelte';
  import { world } from '../stores/world.svelte';
  import { inspectorHeader } from './inspectorHeader';
  import InspectorFrame from './InspectorFrame.svelte';
  import NavRail from './NavRail.svelte';
  import RecordingPicker from './RecordingPicker.svelte';
  import ReplayBar from './ReplayBar.svelte';
  import type { ScrubMarker } from './Scrubber.svelte';
  import SettingsDialog from './SettingsDialog.svelte';
  import ShortcutSheet from './ShortcutSheet.svelte';
  import StatusBar from './StatusBar.svelte';
  import TopBar from './TopBar.svelte';
  import { handleGlobalKey } from './shortcuts';

  let { view, inspector }: { view: Snippet<[Route]>; inspector: Snippet } = $props();

  let width = $state(typeof window !== 'undefined' ? window.innerWidth : 1360);
  const overlay = $derived(width < 1100);
  const replay = $derived(world.mode === 'replay');
  const disabled = $derived<Route[]>(scope.sessionId ? [] : ['graph', 'workflow', 'timeline']);
  const inspectorHead = $derived(inspectorHeader(selection.current));

  // Loading skeleton only after 150 ms (2.10).
  let showSkeleton = $state(false);
  $effect(() => {
    if (world.phase !== 'connecting') {
      showSkeleton = false;
      return;
    }
    const t = setTimeout(() => (showSkeleton = true), 150);
    return () => clearTimeout(t);
  });

  // Pause loops while the window is hidden (5.3, rule 4).
  $effect(() => {
    const vis = () => document.documentElement.setAttribute('data-paused', String(document.visibilityState === 'hidden'));
    document.addEventListener('visibilitychange', vis);
    return () => document.removeEventListener('visibilitychange', vis);
  });

  // A scope that no longer exists falls back to Overview.
  $effect(() => {
    if (world.phase === 'ready' && scope.sessionId && !world.session(scope.sessionId) && world.mode === 'live') {
      scope.openSession(null);
      route.navigate('overview');
    }
    if (!scope.sessionId && route.current !== 'overview') route.current = 'overview';
  });

  // ---------- replay bar data ----------
  const DENSITY_BUCKETS = 300;
  const density = $derived.by(() => {
    if (!replay) return [];
    const [a, b] = clock.range;
    const all: number[] = [];
    for (const times of Object.values(world.activity)) for (const t of times) all.push(t);
    return bucketize(all, a, b + 1, DENSITY_BUCKETS);
  });
  const gaps = $derived.by(() => {
    if (!replay || !clock.skipGaps || !clock.canSkipGaps) return [] as [number, number][];
    const [a, b] = clock.range;
    const times = world.timelineTimes.filter((t) => t >= a && t <= b);
    const out: [number, number][] = [];
    let prev = a;
    for (const t of times) {
      if (t - prev > 30_000) out.push([prev, t]);
      prev = t;
    }
    if (times.length && b - prev > 30_000) out.push([prev, b]);
    return out;
  });
  const markers = $derived.by(() => {
    const s = world.session(scope.sessionId);
    if (!replay || !s) return [] as ScrubMarker[];
    const out: ScrubMarker[] = [];
    for (const run of Object.values(s.runs)) {
      if (run.startedAt != null) out.push({ kind: 'run', t: run.startedAt, end: run.endedAt ?? undefined, label: run.name });
      for (const ph of run.phases) {
        const starts = run.agentIds.map((id) => s.agents[id]).filter((x) => x && x.phaseIndex === ph.index && x.startedAt != null).map((x) => x.startedAt!);
        if (starts.length) out.push({ kind: 'phase', t: Math.min(...starts), label: `${ph.index} ${ph.title}` });
      }
    }
    return out;
  });
  function runningAt(t: number): number {
    let n = 0;
    for (const s of Object.values(world.snapshot.sessions))
      for (const a of Object.values(s.agents))
        if (a.agentId !== MAIN_AGENT_ID && a.startedAt != null && a.startedAt <= t && (a.endedAt ?? Infinity) > t) n += 1;
    return n;
  }

  const showMissing = $derived(world.phase === 'ready' && world.mode === 'live' && !!world.status?.error && world.sessions.length === 0);
</script>

<svelte:window bind:innerWidth={width} onkeydown={handleGlobalKey} />

<IconSprite />

<div class="frame" class:replay class:overlay class:with-inspector={ui.inspectorOpen && !overlay} style="--insp: {settings.value.inspectorWidth}px">
  <NavRail current={route.current} {disabled} onnavigate={(r) => route.navigate(r)} onsettings={() => (ui.settingsOpen = true)} />
  <TopBar />

  <main class="view" aria-label="View">
    {#if world.phase === 'connecting'}
      <div class="loading">
        {#if showSkeleton}
          <div class="skeletons" aria-hidden="true">
            {#each [0, 1, 2] as i (i)}<Skeleton height={168} radius="var(--radius-lg)" />{/each}
          </div>
        {/if}
        <EmptyState icon="sessions" title="Looking for sessions…" text={world.status?.claudeDir ? `Reading ${world.status.claudeDir}` : 'Reading the Claude folder'} />
      </div>
    {:else if world.phase === 'error'}
      <EmptyState icon="error" tone="error" title="Something went wrong" text={world.error ?? 'Unknown error'} action={{ label: 'Try again', icon: 'refresh', onclick: () => world.retry() }} />
    {:else if showMissing}
      <EmptyState
        icon="folder"
        title="No Claude folder found"
        text="Virgil reads the files Claude Code writes in {world.status?.claudeDir ?? '~/.claude'}."
        action={{ label: 'Choose folder…', icon: 'folder', onclick: () => void actions.chooseClaudeDir() }}
      />
    {:else}
      {@render view(route.current)}
    {/if}
  </main>

  {#if replay}
    <ReplayBar {density} {markers} {gaps} {runningAt} />
  {/if}

  <InspectorFrame
    open={ui.inspectorOpen}
    width={settings.value.inspectorWidth}
    title={inspectorHead.title}
    {overlay}
    onclose={() => (ui.inspectorOpen = false)}
    onresize={(w) => settings.update({ inspectorWidth: w })}
  >
    {#snippet header()}
      {#if inspectorHead.glyph}
        <TypeGlyph kind={inspectorHead.glyph.kind} agentType={inspectorHead.glyph.agentType} />
      {:else if inspectorHead.icon}
        <span class="hicon"><Icon name={inspectorHead.icon} size={18} /></span>
      {/if}
      {#if inspectorHead.attempt && inspectorHead.attempt > 1}<span class="attempt t-small">x{inspectorHead.attempt}</span>{/if}
      {#if inspectorHead.state}<StateBadge state={inspectorHead.state} />{/if}
    {/snippet}
    {@render inspector()}
  </InspectorFrame>

  <StatusBar
    source={world.status}
    sourceKind={world.source?.kind ?? null}
    busySessions={world.busy}
    runningAgents={world.runningAgents}
    lastEventAt={world.lastEventAt}
    problems={world.problems}
    now={clock.wall}
    onchoosefolder={() => void actions.chooseClaudeDir()}
  />
</div>

<SettingsDialog open={ui.settingsOpen} onclose={() => (ui.settingsOpen = false)} />
<ShortcutSheet open={ui.shortcutsOpen} onclose={() => (ui.shortcutsOpen = false)} />
<RecordingPicker
  open={ui.pickerOpen}
  recent={settings.value.recentRecordings}
  list={() => world.listRecordings()}
  pick={world.canPickFolder ? () => world.pickRecordingFolder() : undefined}
  error={ui.pickerError}
  onopen={(path, info) => actions.openRecording(path, info)}
  onclose={() => (ui.pickerOpen = false)}
/>

<style>
  .frame {
    position: relative;
    display: grid;
    height: 100vh;
    width: 100vw;
    min-width: 960px;
    min-height: 600px;
    grid-template-columns: var(--size-navrail) minmax(0, 1fr) auto;
    grid-template-rows: var(--size-topbar) minmax(0, 1fr) auto var(--size-statusbar);
    grid-template-areas:
      'rail top insp'
      'rail view insp'
      'rail replay insp'
      'status status status';
    background: var(--surface-app);
  }
  .frame.overlay { grid-template-columns: var(--size-navrail) minmax(0, 1fr) 0; }
  .frame.replay::before {
    content: '';
    position: absolute;
    inset: 0 0 auto 0;
    height: 2px;
    background: var(--state-queued);
    z-index: var(--z-sticky);
    pointer-events: none;
  }
  .view { grid-area: view; position: relative; min-width: 0; min-height: 0; overflow: hidden; display: flex; flex-direction: column; }
  .view > :global(*) { flex: 1; min-height: 0; }
  .overlay :global(.inspector) { grid-area: view; justify-self: end; }
  .loading { position: relative; display: grid; }
  .skeletons {
    position: absolute;
    inset: var(--space-6);
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
    gap: var(--space-4);
    align-content: start;
    opacity: 0.6;
  }
  .hicon { display: inline-grid; place-items: center; color: var(--text-secondary); }
  .attempt { padding: 0 5px; border-radius: var(--radius-xs); background: var(--surface-sunken); color: var(--text-secondary); }
</style>
