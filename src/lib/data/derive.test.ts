import { describe, expect, test } from 'vitest';
import { DEMO_RUN_ID, SCENARIO_DURATION, SESSION_NOTES, SESSION_ORCHARD, buildScenario, scenarioAt } from './scenario';
import { activePhaseIndex, agentsByPhase, bucketize, childrenOf, nowLine, parentChain, parsePhaseKey, phaseKey, runTotals, sessionStats, sharedPairs } from './derive';

const events = buildScenario(0);
const end = scenarioAt(events, SCENARIO_DURATION).sessions[SESSION_ORCHARD];
const mid = scenarioAt(events, 120_000).sessions[SESSION_ORCHARD];

describe('derive', () => {
  test('children: workflow agents are not children; nested agents hang off their parent', () => {
    const kids = childrenOf(end);
    const main = kids.get('main') ?? [];
    expect(main.every((a) => a.runId == null)).toBe(true);
    const nestedParent = [...kids.entries()].find(([p]) => p !== 'main');
    expect(nestedParent?.[1][0].spawnDepth).toBe(2);
  });

  test('agents by phase and totals', () => {
    const run = end.runs[DEMO_RUN_ID];
    const byPhase = agentsByPhase(end, run);
    expect([...byPhase.keys()]).toEqual([1, 2, 3, 4]);
    expect(byPhase.get(2)?.length).toBe(8);
    const totals = runTotals(end, run);
    expect(totals.agents).toBe(20);
    expect(totals.failed).toBe(1);
    expect(totals.durationMs).toBeGreaterThan(0);
  });

  test('active phase and now line mid-run', () => {
    const run = mid.runs[DEMO_RUN_ID];
    expect(activePhaseIndex(mid, run)).toBe(2);
    expect(nowLine(mid)).toMatchObject({ phaseTitle: 'Build', total: 8 });
    expect(nowLine(end)).toBeUndefined();
  });

  test('session stats', () => {
    const st = sessionStats(mid);
    expect(st.runs).toBe(1);
    expect(st.running).toBeGreaterThan(0);
    expect(st.tokens).toBeGreaterThan(0);
    const notes = scenarioAt(events, 0).sessions[SESSION_NOTES];
    expect(sessionStats(notes).agents).toBe(0);
  });

  test('shared pairs', () => {
    const all = sharedPairs(end, 'all');
    const edits = sharedPairs(end, 'edits');
    expect(all.length).toBeGreaterThan(edits.length);
    expect(edits.every((p) => p.bothEdited)).toBe(true);
    expect(all[0].files.length).toBeGreaterThanOrEqual(all.at(-1)!.files.length);
  });

  test('parent chain for a workflow agent and a nested agent', () => {
    const run = end.runs[DEMO_RUN_ID];
    const wf = parentChain(end, run.agentIds[0]);
    expect(wf.map((p) => p.kind)).toEqual(['session', 'run', 'phase', 'agent']);
    const nested = Object.values(end.agents).find((a) => a.spawnDepth === 2)!;
    expect(parentChain(end, nested.agentId).map((p) => p.kind)).toEqual(['session', 'agent', 'agent']);
  });

  test('phase keys and buckets', () => {
    expect(parsePhaseKey(phaseKey('wf_1-2', 3))).toEqual({ runId: 'wf_1-2', index: 3 });
    expect(bucketize([0, 1, 5, 9, 10], 0, 10, 2)).toEqual([2, 2]);
  });
});
