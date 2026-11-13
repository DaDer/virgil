<script lang="ts">
  // ux-spec 3.4: when things happened, how long they waited and ran. Props: none (reads stores).
  // Rows are virtualised; label column and tracks share one vertical scroll container; the time
  // window is a Scale (pan and zoom change it, never a horizontal scroll).
  import { MAIN_AGENT_ID, type Agent } from '../../contract';
  import EmptyState from '../../components/EmptyState.svelte';
  import Icon from '../../components/Icon.svelte';
  import Menu from '../../components/Menu.svelte';
  import MenuItem from '../../components/MenuItem.svelte';
  import ModelChip from '../../components/ModelChip.svelte';
  import StateIcon from '../../components/StateIcon.svelte';
  import { bucketize, sessionTitle } from '../../data/derive';
  import { formatClock, formatCount, formatDuration } from '../../format';
  import { AGENT_STATE, PHASE_STATE, RUN_STATUS, SESSION_STATE, sessionState } from '../../states';
  import { actions } from '../../stores/actions';
  import { clock, SPEEDS } from '../../stores/clock.svelte';
  import { scope } from '../../stores/route.svelte';
  import { search } from '../../stores/search.svelte';
  import { hover, sel, selection } from '../../stores/selection.svelte';
  import { settings } from '../../stores/settings.svelte';
  import { ui } from '../../stores/ui.svelte';
  import { world } from '../../stores/world.svelte';
  import { history } from '../shared/history.svelte';
  import { links } from '../shared/links.svelte';
  import { ticksUntil, type ToolTick } from '../shared/toolHistory';
  import { buildRows, firstTime, phaseBands, visibleRange, type Grouping, type TimelineRow } from './buildRows';
  import TimeAxis from './TimeAxis.svelte';
  import TimeMinimap from './TimeMinimap.svelte';
  import { fit as fitScale, fromWindow, pan, toT, toX, zoomAround, type Scale } from './timeScale';
  import TrackRow from './TrackRow.svelte';

  const session = $derived(world.session(scope.sessionId));
  const sessionId = $derived(session?.sessionId ?? '');
  const replay = $derived(world.mode === 'replay');
  const now = $derived(clock.viewTime);
  const rowH = $derived(settings.value.density === 'compact' ? 24 : 28);

  // ---------- options ----------
  let group = $state<Grouping>('phase');
  let collapsed = $state<Record<string, boolean>>({});
  let follow = $state(true);
  let labelW = $state(260);
  let groupBtn: HTMLButtonElement | undefined = $state();
  let groupOpen = $state(false);
  let speedBtn: HTMLButtonElement | undefined = $state();
  let speedOpen = $state(false);

  const rows = $derived(
    session
      ? buildRows(session, { group, collapsed, scopedRunId: scope.runId ?? world.runOrCurrent(sessionId, null)?.runId ?? null, viewTime: now, hideFuture: replay })
      : [],
  );
  const bands = $derived(phaseBands(rows));
  const hasAgents = $derived(rows.some((r) => r.kind === 'agent') || (session ? Object.keys(session.runs).length > 0 : false));

  // ---------- time range ----------
  const range = $derived.by((): [number, number] => {
    if (!session) return [now - 60_000, now];
    let a = Infinity;
    let b = -Infinity;
    for (const ag of Object.values(session.agents)) {
      if (ag.agentId === MAIN_AGENT_ID) continue;
      const f = firstTime(ag);
      if (Number.isFinite(f)) a = Math.min(a, f);
      if (ag.endedAt != null) b = Math.max(b, ag.endedAt);
    }
    for (const r of Object.values(session.runs)) if (r.startedAt != null) a = Math.min(a, r.startedAt);
    const mainTicks = history.ticks(sessionId, MAIN_AGENT_ID);
    if (mainTicks.length) a = Math.min(a, mainTicks[0].t);
    if (replay) {
      a = Math.min(a, clock.range[0]);
      b = Math.max(b, clock.range[1]);
    } else b = Math.max(b, now);
    if (!Number.isFinite(a)) a = session.startedAt ?? now - 60_000;
    if (!Number.isFinite(b) || b <= a) b = a + 60_000;
    return [a, b];
  });
  const origin = $derived(replay ? clock.range[0] : (session?.startedAt ?? range[0]));

  // ---------- scale ----------
  let trackW = $state(800);
  let scale = $state<Scale | null>(null);
  const live = $derived(!replay);
  const view = $derived.by((): Scale => {
    const s = scale ?? fitScale(range[0], range[1], trackW);
    if (live && follow) return { t0: now - (trackW - 24) * s.msPerPx, msPerPx: s.msPerPx };
    return s;
  });

  function setScale(s: Scale) {
    follow = false;
    scale = s;
  }
  function doFit() {
    scale = fitScale(range[0], range[1], trackW);
    if (live) follow = true;
  }
  // Fit once the track width is known, and again when the session or mode changes.
  let fittedFor = '';
  $effect(() => {
    const key = `${sessionId}:${world.mode}:${world.status?.recording?.path ?? ''}`;
    if (trackW > 0 && key !== fittedFor) {
      fittedFor = key;
      scale = fitScale(range[0], range[1], trackW);
      follow = live;
    }
  });

  // ---------- virtualisation ----------
  let scroller: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewH = $state(400);
  const slice = $derived(visibleRange(rows.length, rowH, scrollTop, viewH));
  const shown = $derived(rows.slice(slice[0], slice[1]).map((r, i) => ({ row: r, i: slice[0] + i })));

  // ---------- data per row ----------
  function agentOf(r: TimelineRow): Agent | null {
    if (r.kind === 'agent') return session?.agents[r.agentId] ?? null;
    if (r.kind === 'session') return session?.agents[MAIN_AGENT_ID] ?? null;
    return null;
  }
  function ticksOf(r: TimelineRow): readonly ToolTick[] {
    if (!session) return [];
    const id = r.kind === 'agent' ? r.agentId : r.kind === 'session' ? MAIN_AGENT_ID : null;
    if (!id) return [];
    const all = history.ticks(sessionId, id);
    return replay ? ticksUntil(all, now) : all;
  }
  const tickTimesCache = new WeakMap<readonly ToolTick[], number[]>();
  function times(ts: readonly ToolTick[]): number[] {
    let t = tickTimesCache.get(ts);
    if (!t) tickTimesCache.set(ts, (t = ts.map((x) => x.t)));
    return t;
  }

  function isSelected(r: TimelineRow): boolean {
    if (r.kind === 'agent') return selection.is('agent', r.agentId, sessionId);
    if (r.kind === 'session') return selection.is('session', sessionId);
    if (r.kind === 'run') return selection.is('run', r.runId, sessionId);
    return selection.is('phase', `${r.runId}:${r.index}`, sessionId);
  }
  function selectRow(r: TimelineRow) {
    if (r.kind === 'agent') actions.select(sel.agent(sessionId, r.agentId));
    else if (r.kind === 'session') actions.select(sel.session(sessionId));
    else if (r.kind === 'run') actions.select(sel.run(sessionId, r.runId));
    else actions.select(sel.phase(sessionId, r.runId, r.index));
  }
  function toggle(r: TimelineRow) {
    if (r.kind !== 'run' && r.kind !== 'phase') return;
    collapsed = { ...collapsed, [r.id]: !r.collapsed };
  }

  // ---------- pointer on tracks ----------
  let tip = $state<{ x: number; y: number; lines: string[]; title: string } | null>(null);

  function rowAt(clientY: number): { row: TimelineRow; i: number } | null {
    if (!scroller) return null;
    const r = scroller.getBoundingClientRect();
    const i = Math.floor((clientY - r.top + scroller.scrollTop) / rowH);
    return rows[i] ? { row: rows[i], i } : null;
  }
  function trackX(clientX: number): number {
    const r = scroller!.getBoundingClientRect();
    return clientX - r.left - labelW;
  }

  function onTrackMove(e: PointerEvent) {
    if (dragHead) return;
    const hit = rowAt(e.clientY);
    const x = trackX(e.clientX);
    if (!hit || x < 0) {
      tip = null;
      return;
    }
    const r = hit.row;
    const ts = ticksOf(r);
    let near: ToolTick | null = null;
    let best = 3.01;
    for (const t of ts) {
      const d = Math.abs(toX(view, t.t) - x);
      if (d < best) {
        best = d;
        near = t;
      }
    }
    if (near) {
      const dur = near.endedAt != null ? ` · ${formatDuration(near.endedAt - near.t, { allowMs: true })}` : '';
      tip = { x: e.clientX, y: e.clientY, title: near.name, lines: [near.summary || '–', `${formatClock(near.t)}${dur}${near.isError ? ' · failed' : ''}`] };
      return;
    }
    const a = agentOf(r);
    if (a && r.kind === 'agent') {
      const t = toT(view, x);
      const s0 = a.queuedAt ?? a.startedAt;
      const e0 = a.endedAt ?? now;
      if (s0 != null && t >= s0 - view.msPerPx * 3 && t <= e0 + view.msPerPx * 3) {
        const waited = a.queuedAt != null && a.startedAt != null ? formatDuration(a.startedAt - a.queuedAt) : '–';
        const ran = a.startedAt != null ? formatDuration(Math.max(0, (a.endedAt ?? now) - a.startedAt)) : '–';
        tip = {
          x: e.clientX,
          y: e.clientY,
          title: a.label,
          lines: [`${AGENT_STATE[a.state].label} · waited ${waited} · ran ${ran}`, `${a.tokens > 0 ? formatCount(a.tokens) : '–'} tok · ${a.toolCalls} tool calls`],
        };
        hover.set(sel.agent(sessionId, a.agentId));
        return;
      }
    }
    tip = null;
  }
  function onTrackLeave() {
    tip = null;
    hover.clear();
  }
  function onTrackClick(e: MouseEvent) {
    const hit = rowAt(e.clientY);
    const x = trackX(e.clientX);
    if (x < 0) return;
    if (hit) {
      const a = agentOf(hit.row);
      const t = toT(view, x);
      const onBar = a && hit.row.kind === 'agent' && (a.queuedAt ?? a.startedAt) != null && t >= (a.queuedAt ?? a.startedAt)! - view.msPerPx * 3 && t <= (a.endedAt ?? now) + view.msPerPx * 3;
      if (onBar || hit.row.kind !== 'agent') {
        if (hit.row.kind !== 'session' || !replay) return selectRow(hit.row);
      }
    }
    if (replay) void clock.seek(toT(view, x));
  }

  function onWheel(e: WheelEvent) {
    const x = trackX(e.clientX);
    if (e.ctrlKey || e.metaKey) {
      e.preventDefault();
      setScale(zoomAround(view, Math.exp(e.deltaY * 0.01), Math.max(0, x)));
    } else if (e.shiftKey || Math.abs(e.deltaX) > Math.abs(e.deltaY)) {
      e.preventDefault();
      setScale(pan(view, e.shiftKey && !e.deltaX ? e.deltaY : e.deltaX));
    }
  }

  // ---------- playhead ----------
  const headX = $derived(toX(view, now));
  let dragHead = $state(false);
  function headDown(e: PointerEvent) {
    if (!replay) return;
    e.preventDefault();
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    dragHead = true;
  }
  function headMove(e: PointerEvent) {
    if (dragHead) clock.scrub(toT(view, trackX(e.clientX)));
  }
  function headUp(e: PointerEvent) {
    if (!dragHead) return;
    dragHead = false;
    void clock.endScrub(toT(view, trackX(e.clientX)));
  }

  // ---------- label column resize ----------
  function resizeDown(e: PointerEvent) {
    e.preventDefault();
    const x0 = e.clientX;
    const w0 = labelW;
    const move = (ev: PointerEvent) => (labelW = Math.min(400, Math.max(180, w0 + ev.clientX - x0)));
    const up = () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
  }

  // ---------- minimap ----------
  const density = $derived(session ? bucketize(world.activityOf(sessionId), range[0], range[1] + 1, 120) : []);
  const runSpans = $derived(session ? Object.values(session.runs).filter((r) => r.startedAt != null).map((r) => ({ start: r.startedAt!, end: r.endedAt ?? now })) : []);
  const win = $derived<[number, number]>([view.t0, toT(view, trackW)]);

  // ---------- keyboard and focus requests ----------
  let root: HTMLElement | undefined = $state();
  function onkey(e: KeyboardEvent) {
    if (e.defaultPrevented || e.metaKey || e.ctrlKey || e.altKey || ui.anyDialog) return;
    const t = e.target as HTMLElement;
    if (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA') return;
    const k = e.key;
    const done = () => e.preventDefault();
    if (k === 'f' || k === 'F') return done(), doFit();
    if (k === '+' || k === '=') return done(), setScale(zoomAround(view, 0.6, trackW / 2));
    if (k === '-' || k === '_') return done(), setScale(zoomAround(view, 1 / 0.6, trackW / 2));
    if (k === '0') return done(), doFit();
    if (live && k === 'Home') return done(), setScale({ t0: range[0] - 24 * view.msPerPx, msPerPx: view.msPerPx });
    if (live && k === 'End') return done(), (scale = view), (follow = true);
    if ((k === 'ArrowUp' || k === 'ArrowDown') && root?.contains(document.activeElement)) {
      done();
      const agentRows = rows.map((r, i) => ({ r, i })).filter((x) => x.r.kind === 'agent' || x.r.kind === 'session');
      const cur = agentRows.findIndex((x) => isSelected(x.r));
      const next = agentRows[cur < 0 ? 0 : Math.max(0, Math.min(agentRows.length - 1, cur + (k === 'ArrowDown' ? 1 : -1)))];
      if (next) {
        selectRow(next.r);
        scrollToRow(next.i);
      }
    }
  }
  function scrollToRow(i: number) {
    if (!scroller) return;
    const top = i * rowH;
    if (top < scroller.scrollTop) scroller.scrollTop = top;
    else if (top + rowH > scroller.scrollTop + viewH) scroller.scrollTop = top + rowH - viewH;
  }
  $effect(() => {
    const req = links.timelineFocus;
    if (!req || req.sessionId !== sessionId) return;
    const i = rows.findIndex((r) => r.kind === 'agent' && r.agentId === req.agentId);
    if (i < 0) return;
    links.timelineFocus = null;
    actions.select(sel.agent(sessionId, req.agentId));
    queueMicrotask(() => {
      if (scroller) scroller.scrollTop = Math.max(0, i * rowH - viewH / 3);
    });
  });

  const groupLabel: Record<Grouping, string> = { phase: 'Phase', start: 'Start time', state: 'State' };
</script>

<svelte:window onkeydowncapture={onkey} />

<section class="view" aria-label="Timeline" bind:this={root} tabindex="-1">
  {#if !session}
    <EmptyState icon="timeline" title="No session in scope" text="Pick a session on the Overview first." />
  {:else}
    <div class="toolbar" role="toolbar" aria-label="Timeline tools">
      <button type="button" class="tb" onclick={doFit} title="Fit (F)">Fit</button>
      <button type="button" class="tb step" aria-label="Zoom out" title="Zoom out (-)" onclick={() => setScale(zoomAround(view, 1 / 0.6, trackW / 2))}>−</button>
      <button type="button" class="tb step" aria-label="Zoom in" title="Zoom in (+)" onclick={() => setScale(zoomAround(view, 0.6, trackW / 2))}>+</button>
      <span class="sep"></span>
      <button type="button" class="tb" bind:this={groupBtn} aria-haspopup="menu" aria-expanded={groupOpen} onclick={() => (groupOpen = !groupOpen)}>Group: {groupLabel[group]} <Icon name="chevron-down" size={14} /></button>
      <Menu anchor={groupBtn} open={groupOpen} onclose={() => (groupOpen = false)} label="Group rows by">
        {#each ['phase', 'start', 'state'] as const as g (g)}
          <MenuItem checked={group === g} onselect={() => ((groupOpen = false), (group = g))}>{groupLabel[g]}</MenuItem>
        {/each}
      </Menu>
      {#if live}
        <label class="tb check"><input type="checkbox" checked={follow} onchange={(e) => (e.currentTarget.checked ? ((scale = view), (follow = true)) : (follow = false))} /> Follow</label>
      {:else}
        <span class="sep"></span>
        <button type="button" class="tb play" onclick={() => actions.togglePlay()} title="Play or pause (Space)">
          <Icon name={clock.playing ? 'pause' : 'play'} size={14} />{clock.playing ? 'Pause' : 'Play'}
        </button>
        <button type="button" class="tb num" bind:this={speedBtn} aria-haspopup="menu" aria-expanded={speedOpen} onclick={() => (speedOpen = !speedOpen)} title="Speed ([ and ])">{clock.speed}x</button>
        <Menu anchor={speedBtn} open={speedOpen} onclose={() => (speedOpen = false)} label="Replay speed">
          {#each SPEEDS as sp (sp)}
            <MenuItem checked={clock.speed === sp} onselect={() => ((speedOpen = false), void clock.setSpeed(sp))}>{sp}x</MenuItem>
          {/each}
        </Menu>
      {/if}
      <span class="flex"></span>
      <span class="count t-small num">{rows.length - 1} rows</span>
    </div>

    <div class="head" style="--label-w: {labelW}px">
      <div class="corner">
        <span class="t-label">{session ? sessionTitle(session) : ''}</span>
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="resize" onpointerdown={resizeDown} title="Resize the label column"></div>
      </div>
      <div class="trackhead" bind:clientWidth={trackW}>
        <TimeMinimap {range} window={win} {density} runs={runSpans} width={trackW} onwindow={(a, b) => setScale(fromWindow(a, b, trackW))} />
        <TimeAxis scale={view} width={trackW} mode={settings.value.timeAxis === 'clock' ? 'clock' : 'elapsed'} {origin} onrange={(a, b) => setScale(fromWindow(a, b, trackW))} />
      </div>
    </div>

    <div class="body">
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
      <div
        class="scroller"
        bind:this={scroller}
        bind:clientHeight={viewH}
        onscroll={() => (scrollTop = scroller?.scrollTop ?? 0)}
        onwheel={onWheel}
        onpointermove={onTrackMove}
        onpointerleave={onTrackLeave}
        onclick={onTrackClick}
        style="--label-w: {labelW}px; --row-h: {rowH}px"
      >
        <div class="inner" style="height: {rows.length * rowH}px">
          {#each bands as b (b.id + b.from)}
            {@const ph = rows.find((r) => r.id === b.id)}
            {#if ph && ph.kind === 'phase' && ph.start != null}
              {@const x0 = Math.max(0, toX(view, ph.start))}
              {@const x1 = Math.min(trackW, toX(view, ph.end ?? now))}
              {#if x1 > x0}
                <div class="band" style="top: {b.from * rowH}px; height: {(b.to - b.from + 1) * rowH}px; left: calc(var(--label-w) + {x0}px); width: {x1 - x0}px"></div>
              {/if}
            {/if}
          {/each}
          {#each shown as { row, i } (row.id)}
            {@const a = agentOf(row)}
            {@const selected = isSelected(row)}
            {@const dim = row.kind === 'agent' && !search.isMatch(sessionId, row.agentId)}
            <div class="row" class:selected class:dim class:group={row.kind === 'run' || row.kind === 'phase'} class:hovered={row.kind === 'agent' && hover.is('agent', row.agentId, sessionId)} style="transform: translateY({i * rowH}px)">
              <div class="label" style="padding-left: {8 + row.depth * 16}px">
                {#if row.kind === 'run' || row.kind === 'phase'}
                  <button type="button" class="chev" aria-label={row.collapsed ? 'Expand' : 'Collapse'} aria-expanded={!row.collapsed} onclick={(e) => (e.stopPropagation(), toggle(row))}>
                    <Icon name={row.collapsed ? 'chevron-right' : 'chevron-down'} size={14} />
                  </button>
                {/if}
                <button type="button" class="lbl" onclick={(e) => (e.stopPropagation(), selectRow(row))}>
                  {#if row.kind === 'session' && session}
                    <StateIcon state={SESSION_STATE[sessionState(session)]} size={14} />
                    <span class="name t-strong ellipsis">{sessionTitle(session)}</span>
                  {:else if row.kind === 'agent' && a}
                    <StateIcon state={a.state} size={14} animated={a.state === 'running'} />
                    <span class="name ellipsis" title={a.label}>{a.label}</span>
                    <ModelChip model={a.model} compact />
                  {:else if row.kind === 'run'}
                    {@const r = session.runs[row.runId]}
                    {#if r}<StateIcon state={RUN_STATUS[r.status]} size={14} />{/if}
                    <span class="name t-strong ellipsis">{r?.name ?? row.runId}</span>
                    <span class="meta num">{row.count}</span>
                  {:else if row.kind === 'phase'}
                    {@const ph = session.runs[row.runId]?.phases.find((p) => p.index === row.index)}
                    {#if ph}<StateIcon state={PHASE_STATE[ph.state]} size={14} />{/if}
                    <span class="name ellipsis"><span class="num">{row.index}</span> {row.title}</span>
                    <span class="meta num">{row.done}/{row.total}</span>
                  {/if}
                </button>
              </div>
              <TrackRow
                agent={a}
                span={row.kind === 'run' || row.kind === 'phase' ? { start: row.start, end: row.end } : undefined}
                kind={row.kind}
                ticks={times(ticksOf(row))}
                scale={view}
                width={trackW}
                height={rowH}
                viewTime={now}
                {selected}
              />
            </div>
          {/each}
        </div>
      </div>

      {#if headX >= -2 && headX <= trackW + 2}
        <div class="playhead" class:replay class:dragging={dragHead} style="left: calc({labelW}px + {headX}px)">
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="flag t-small num" onpointerdown={headDown} onpointermove={headMove} onpointerup={headUp}>{replay ? formatClock(now) : 'Now'}</div>
        </div>
      {/if}

      {#if !hasAgents}
        <div class="empty"><EmptyState icon="timeline" title="Just the session so far" text="This session has not started any agents." /></div>
      {/if}
    </div>

    {#if tip}
      <div class="tip t-small" style="left: {tip.x + 14}px; top: {tip.y + 14}px" role="tooltip">
        <p class="t-strong">{tip.title}</p>
        {#each tip.lines as l, i (i)}<p class="secondary">{l}</p>{/each}
      </div>
    {/if}
  {/if}
</section>

<style>
  .view { position: relative; height: 100%; display: flex; flex-direction: column; min-height: 0; background: var(--surface-canvas); }
  .view:focus { outline: none; }
  .toolbar { display: flex; align-items: center; gap: var(--space-1); padding: var(--space-2) var(--space-3); background: var(--surface-panel); border-bottom: var(--border-width) solid var(--border-subtle); }
  .tb { display: inline-flex; align-items: center; gap: 4px; height: var(--size-control); padding: 0 var(--space-2); border-radius: var(--radius-sm); color: var(--text-primary); font-size: var(--text-sm); }
  .tb:hover { background: var(--surface-hover); }
  .tb.step { width: var(--size-control); justify-content: center; font-size: var(--text-lg); color: var(--text-secondary); }
  .tb.play { background: var(--accent); color: var(--text-on-accent); }
  .tb.play:hover { background: var(--accent-hover); }
  .check { cursor: pointer; }
  .check input { accent-color: var(--accent); margin: 0; }
  .sep { width: 1px; height: 18px; background: var(--border-subtle); margin: 0 var(--space-1); }
  .flex { flex: 1; }
  .count { color: var(--text-muted); }

  .head { display: flex; border-bottom: var(--border-width) solid var(--border-default); background: var(--surface-panel); }
  .corner { position: relative; flex: none; width: var(--label-w); display: flex; align-items: flex-end; padding: 0 var(--space-3) 6px; color: var(--text-muted); border-right: var(--border-width) solid var(--border-subtle); overflow: hidden; }
  .resize { position: absolute; right: -3px; top: 0; bottom: 0; width: 6px; cursor: col-resize; z-index: 1; }
  .resize:hover { background: var(--accent-soft); }
  .trackhead { flex: 1; min-width: 0; overflow: hidden; }

  .body { position: relative; flex: 1; min-height: 0; }
  .scroller { position: absolute; inset: 0; overflow-y: auto; overflow-x: hidden; }
  .inner { position: relative; }
  .band { position: absolute; background: color-mix(in srgb, var(--surface-hover) 50%, transparent); pointer-events: none; }
  .row { position: absolute; left: 0; right: 0; top: 0; height: var(--row-h); display: flex; border-bottom: var(--border-width) solid color-mix(in srgb, var(--border-subtle) 60%, transparent); }
  .row.group { background: color-mix(in srgb, var(--surface-panel) 55%, transparent); }
  .row.hovered { background: var(--surface-hover); }
  .row.selected { background: var(--surface-selected); }
  .row.selected .label { box-shadow: inset 2px 0 0 var(--select-ring); }
  .row.dim { opacity: 0.25; }
  .label { flex: none; width: var(--label-w); display: flex; align-items: center; gap: 2px; padding-right: var(--space-2); background: var(--surface-panel); border-right: var(--border-width) solid var(--border-subtle); min-width: 0; }
  .chev { flex: none; display: inline-grid; place-items: center; width: 18px; height: 18px; border-radius: var(--radius-xs); color: var(--text-secondary); }
  .chev:hover { background: var(--surface-hover); }
  .lbl { flex: 1; min-width: 0; display: flex; align-items: center; gap: 6px; height: 100%; text-align: left; color: var(--text-primary); font-size: var(--text-sm); }
  .name { flex: 1; min-width: 0; }
  .meta { color: var(--text-muted); font-size: var(--text-xs); }

  .playhead { position: absolute; top: 0; bottom: 0; width: 1px; background: var(--state-running); pointer-events: none; z-index: 2; }
  .playhead.replay { background: var(--accent); }
  .flag { position: absolute; top: 0; left: 0; transform: translateX(-50%); padding: 0 6px; border-radius: var(--radius-xs); background: var(--surface-raised); border: var(--border-width) solid var(--state-running); color: var(--state-running-text); font-size: var(--text-xs); white-space: nowrap; pointer-events: auto; }
  .replay .flag { background: var(--accent); color: var(--text-on-accent); cursor: ew-resize; }
  .empty { position: absolute; inset: calc(var(--row-h, 28px) * 2) 0 0 0; display: grid; pointer-events: none; }
  .tip { position: fixed; z-index: var(--z-popover); max-width: 340px; padding: var(--space-2) var(--space-3); background: var(--surface-raised); border: var(--border-width) solid var(--border-default); border-radius: var(--radius-sm); box-shadow: var(--shadow-3); pointer-events: none; }
</style>
