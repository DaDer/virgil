import { describe, expect, test } from 'vitest';
import type { VirgilEvent, Snapshot, SourceStatus } from '../contract';
import { LIVE_START_OFFSET, MOCK_RECORDING_BASE, MOCK_RECORDING_PATH, MockSource } from './mockSource';
import { applyEvents } from './reducer';
import { DEMO_RUN_ID, SCENARIO_DURATION, SESSION_ORCHARD, buildScenario, scenarioAt } from './scenario';

function harness(start = 1_800_000_000_000) {
  let wall = start;
  const src = new MockSource({ now: () => wall, timers: false });
  const events: VirgilEvent[] = [];
  const statuses: SourceStatus[] = [];
  return {
    src,
    events,
    statuses,
    advance(ms: number, step = 100) {
      for (let i = 0; i < ms; i += step) {
        wall += step;
        src.tick();
      }
    },
    async connect() {
      await src.connect({ onEvents: (b) => events.push(...b), onStatus: (s) => statuses.push(s) });
    },
  };
}

describe('scenario', () => {
  test('is deterministic', () => {
    expect(JSON.stringify(buildScenario(0))).toBe(JSON.stringify(buildScenario(0)));
  });

  test('ends with a finalised four-phase run of about twenty agents', () => {
    const events = buildScenario(0);
    const end = scenarioAt(events, SCENARIO_DURATION);
    const s = end.sessions[SESSION_ORCHARD];
    const run = s.runs[DEMO_RUN_ID];
    expect(Object.keys(end.sessions)).toHaveLength(5);
    expect(run.final).toBe(true);
    expect(run.phases.map((p) => p.title)).toEqual(['Plan', 'Build', 'Review', 'Fix']);
    expect(run.agentIds.length).toBeGreaterThanOrEqual(20);
    const agents = run.agentIds.map((id) => s.agents[id]);
    expect(agents.filter((a) => a.state === 'failed').length).toBe(2);
    expect(agents.filter((a) => a.superseded).length).toBe(1);
    expect(agents.every((a) => a.state !== 'running')).toBe(true);
    // nested subagent
    expect(Object.values(s.agents).some((a) => a.spawnDepth === 2)).toBe(true);
    // shared files exist
    expect(Object.values(s.files).some((f) => Object.keys(f.touches).length > 1)).toBe(true);
  });

  test('mid-run has running agents and a running phase', () => {
    const events = buildScenario(0);
    const mid = scenarioAt(events, 120_000);
    const run = mid.sessions[SESSION_ORCHARD].runs[DEMO_RUN_ID];
    expect(run.status).toBe('running');
    expect(run.phases.find((p) => p.title === 'Build')?.state).toBe('running');
    expect(run.phases.find((p) => p.title === 'Fix')?.state).toBe('pending');
  });

  test('event seq is 1..n and every event applies to an existing target', () => {
    const events = buildScenario(0);
    events.forEach((e, i) => expect(e.seq).toBe(i + 1));
    for (let i = 1; i < events.length; i++) expect(events[i].t).toBeGreaterThanOrEqual(events[i - 1].t);
  });
});

describe('mock source', () => {
  test('live: snapshot plus streamed events equals the source snapshot', async () => {
    const h = harness();
    await h.connect();
    const first = await h.src.getSnapshot();
    expect(Object.keys(first.sessions).length).toBe(5);
    h.advance(20_000);
    let mine: Snapshot = first;
    mine = applyEvents(mine, h.events.filter((e) => e.seq > first.seq));
    const now = await h.src.getSnapshot();
    expect(mine).toEqual(now);
    expect(now.seq).toBeGreaterThan(first.seq);
  });

  test('live: loops with a snapshotReset', async () => {
    const h = harness();
    await h.connect();
    h.advance(SCENARIO_DURATION - LIVE_START_OFFSET + 1000, 1000);
    expect(h.events.some((e) => e.type === 'snapshotReset')).toBe(true);
    const seqs = h.events.map((e) => e.seq);
    expect(seqs).toEqual([...seqs].sort((a, b) => a - b));
  });

  test('replay controls reject in live mode', async () => {
    const h = harness();
    await expect(h.src.play()).rejects.toMatchObject({ code: 'notReplaying' });
  });

  test('replay: open, play, seek back restores the earlier picture', async () => {
    const h = harness();
    await h.connect();
    await expect(h.src.openRecording('/nope')).rejects.toMatchObject({ code: 'invalidRecording' });
    const opened = await h.src.openRecording(MOCK_RECORDING_PATH);
    expect(opened.replay).toMatchObject({ playing: false, position: MOCK_RECORDING_BASE });
    await h.src.setSpeed(60);
    await h.src.setSkipGaps(false);
    await h.src.play();
    h.advance(2000);
    const st = await h.src.getStatus();
    expect(st.replay!.position).toBeCloseTo(MOCK_RECORDING_BASE + 120_000, -3);
    const at120 = await h.src.getSnapshot();
    await h.src.seek(MOCK_RECORDING_BASE + 60_000);
    const at60 = await h.src.getSnapshot();
    const expected = scenarioAt(buildScenario(MOCK_RECORDING_BASE), MOCK_RECORDING_BASE + 60_000);
    const ids = (s: Snapshot) => Object.keys(s.sessions[SESSION_ORCHARD].agents).sort();
    expect(ids(at60)).toEqual(ids(expected));
    expect(ids(at120).length).toBeGreaterThan(ids(at60).length);
    expect(Object.values(at60.sessions).every((s) => s.liveness === 'unknown')).toBe(true);
    expect(h.events.at(-1)?.type).toBe('snapshotReset');
  });

  test('replay: stops at the end and play restarts from the start', async () => {
    const h = harness();
    await h.connect();
    await h.src.openRecording(MOCK_RECORDING_PATH);
    await h.src.setSpeed(600);
    await h.src.play();
    h.advance(2000);
    let st = await h.src.getStatus();
    expect(st.replay).toMatchObject({ playing: false, position: st.replay!.end });
    st = await h.src.play();
    expect(st.replay).toMatchObject({ playing: true, position: st.replay!.start });
  });

  test('tool history holds every started call up to the position, more than the snapshot', async () => {
    const h = harness();
    await h.connect();
    await h.src.openRecording(MOCK_RECORDING_PATH);
    await h.src.seek(MOCK_RECORDING_BASE + 120_000);
    const snap = await h.src.getSnapshot();
    const hist = await h.src.getToolHistory();
    expect(hist.seq).toBe(snap.seq);
    const total = hist.agents.reduce((n, a) => n + a.calls.length, 0);
    const recent = Object.values(snap.sessions).flatMap((s) => Object.values(s.agents)).reduce((n, a) => n + a.recentTools.length, 0);
    expect(total).toBeGreaterThan(recent);
    expect(hist.agents.every((a) => a.calls.every((c) => c.startedAt <= MOCK_RECORDING_BASE + 120_000))).toBe(true);
    expect(hist.timelineTimes.length).toBeGreaterThan(0);
    expect((await h.src.getStatus()).replay?.skipGaps).toBe(true);
  });

  test('variants: empty has no sessions, missing reports an error', async () => {
    const empty = new MockSource({ variant: 'empty', timers: false });
    expect(Object.keys((await empty.getSnapshot()).sessions)).toHaveLength(0);
    const missing = new MockSource({ variant: 'missing', timers: false });
    expect((await missing.getStatus()).error).toMatch(/not found/);
  });
});
