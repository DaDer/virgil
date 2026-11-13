<script lang="ts">
  // ux-spec 3.1: which sessions are alive, pick one.
  import type { Session } from '../contract';
  import EmptyState from '../components/EmptyState.svelte';
  import { bucketize, compareSessionsBusyFirst, lastActivity, nowLine, sessionStats, sessionTitle } from '../data/derive';
  import { projectName } from '../format';
  import { sessionState } from '../states';
  import { actions } from '../stores/actions';
  import { clock } from '../stores/clock.svelte';
  import { search } from '../stores/search.svelte';
  import { sel, selection } from '../stores/selection.svelte';
  import { world } from '../stores/world.svelte';
  import NowPanel from './NowPanel.svelte';
  import OverviewToolbar, { type OverviewSort } from './OverviewToolbar.svelte';
  import SessionCard from './SessionCard.svelte';
  import SessionSection from './SessionSection.svelte';

  const WEEK = 7 * 24 * 3_600_000;

  let project = $state<string | null>(null);
  let sort = $state<OverviewSort>('busy');

  const all = $derived(world.sessions);
  const projects = $derived.by(() => {
    const m = new Map<string, number>();
    for (const s of all) m.set(projectName(s.cwd), (m.get(projectName(s.cwd)) ?? 0) + 1);
    return [...m.entries()].map(([name, count]) => ({ name, count })).sort((a, b) => b.count - a.count || a.name.localeCompare(b.name));
  });

  const sorter = $derived(
    sort === 'busy'
      ? compareSessionsBusyFirst
      : sort === 'activity'
        ? (a: Session, b: Session) => lastActivity(b) - lastActivity(a)
        : (a: Session, b: Session) => sessionTitle(a).localeCompare(sessionTitle(b)),
  );

  const filtered = $derived(all.filter((s) => project == null || projectName(s.cwd) === project));
  const running = $derived(filtered.filter((s) => sessionState(s) !== 'stale').sort(sorter));
  const recent = $derived(
    filtered
      .filter((s) => sessionState(s) === 'stale' && clock.viewTime - lastActivity(s) < WEEK)
      .sort((a, b) => lastActivity(b) - lastActivity(a)),
  );
  const noMatch = $derived(search.active && filtered.every((s) => !search.isMatch(s.sessionId)));

  function activity(s: Session): number[] {
    const times = world.activityOf(s.sessionId);
    if (sessionState(s) !== 'stale') {
      const end = clock.viewTime;
      return bucketize(times, end - 30 * 60_000, end + 1, 30);
    }
    const start = s.startedAt ?? times[0] ?? 0;
    return bucketize(times, start, lastActivity(s) + 1, 30);
  }
</script>

<div class="overview">
  {#if all.length === 0}
    <EmptyState
      icon="sessions"
      title="No sessions yet"
      text="Start Claude Code in a terminal and it will show up here."
      action={{ label: 'Open a recording…', icon: 'replay', onclick: () => actions.chooseReplay() }}
    />
  {:else}
    <div class="page">
      <header class="head">
        <h1 class="t-display">Sessions</h1>
        <OverviewToolbar {projects} {project} {sort} onproject={(p) => (project = p)} onsort={(s) => (sort = s)} />
      </header>
      <NowPanel />
      {#if noMatch}
        <EmptyState icon="search" title={`Nothing matches "${search.text.trim()}"`} action={{ label: 'Clear search', onclick: () => search.clear() }} />
      {:else}
        {#snippet card(s: Session)}
          <SessionCard
            session={s}
            stats={sessionStats(s)}
            activity={activity(s)}
            now={nowLine(s)}
            selected={selection.is('session', s.sessionId)}
            dimmed={!search.isMatch(s.sessionId)}
            onselect={() => actions.select(sel.session(s.sessionId))}
            onopen={(to) => actions.openSession(s.sessionId, to)}
          />
        {/snippet}
        {#if running.length}
          <SessionSection title="Running now" sessions={running} {card} />
        {/if}
        {#if recent.length}
          <SessionSection title="Recent" sessions={recent} limit={24} {card} />
        {/if}
        {#if !running.length && !recent.length}
          <EmptyState icon="filter" title="No sessions in this project" action={{ label: 'All projects', onclick: () => (project = null) }} />
        {/if}
      {/if}
    </div>
  {/if}
</div>

<style>
  .overview { height: 100%; overflow: auto; display: flex; flex-direction: column; }
  .page { display: flex; flex-direction: column; gap: var(--space-6); padding: var(--space-6); max-width: 1880px; width: 100%; }
  .head { display: flex; align-items: center; justify-content: space-between; gap: var(--space-4); flex-wrap: wrap; }
</style>
