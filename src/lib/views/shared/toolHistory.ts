// Every tool call the UI knows, per agent. The snapshot keeps only an agent's newest
// RECENT_TOOLS_MAX calls (data-contract 2.4), so the world store fills this from the core's
// `get_tool_history` after every snapshot, and keeps it current from `toolCallStarted` and
// `toolCallFinished` events. Pure: no runes.

import type { VirgilEvent, ToolCall } from '../../contract';

export interface ToolTick {
  toolUseId: string;
  name: string;
  summary: string;
  t: number;
  endedAt: number | null;
  isError: boolean | null;
}

/** Same cap as the core's TOOL_HISTORY_MAX. */
const PER_AGENT_MAX = 2_000;

export class ToolHistory {
  private byAgent = new Map<string, Map<string, ToolTick>>();
  private sorted = new Map<string, ToolTick[]>();

  clear(): void {
    this.byAgent.clear();
    this.sorted.clear();
  }

  /** Merge calls of one agent (from the core's history or a snapshot). Returns true when anything changed. */
  addCalls(sessionId: string, agentId: string, calls: readonly ToolCall[]): boolean {
    if (!calls.length) return false;
    const key = `${sessionId}/${agentId}`;
    let m = this.byAgent.get(key);
    if (!m) this.byAgent.set(key, (m = new Map()));
    let changed = false;
    for (const c of calls) {
      const old = m.get(c.toolUseId);
      // A finished call never goes back to unfinished (an older history arriving late).
      if (old && (old.endedAt === c.endedAt || c.endedAt == null) && (old.isError === c.isError || c.isError == null)) continue;
      m.set(c.toolUseId, { toolUseId: c.toolUseId, name: c.name, summary: c.summary, t: c.startedAt, endedAt: c.endedAt ?? old?.endedAt ?? null, isError: c.isError ?? old?.isError ?? null });
      changed = true;
    }
    if (changed) {
      this.trim(m);
      this.sorted.delete(key);
    }
    return changed;
  }

  /** Apply tool events from the stream. Returns true when anything changed. */
  applyEvents(events: readonly VirgilEvent[]): boolean {
    let changed = false;
    for (const e of events) {
      if (e.type === 'toolCallStarted') {
        changed = this.addCalls(e.sessionId, e.agentId, [e.call]) || changed;
      } else if (e.type === 'toolCallFinished') {
        const key = `${e.sessionId}/${e.agentId}`;
        const old = this.byAgent.get(key)?.get(e.toolUseId);
        if (old && (old.endedAt !== e.t || old.isError !== e.isError)) {
          this.byAgent.get(key)!.set(e.toolUseId, { ...old, endedAt: e.t, isError: e.isError });
          this.sorted.delete(key);
          changed = true;
        }
      }
    }
    return changed;
  }

  private trim(m: Map<string, ToolTick>): void {
    if (m.size <= PER_AGENT_MAX) return;
    const keep = [...m.values()].sort((x, y) => x.t - y.t).slice(-PER_AGENT_MAX);
    m.clear();
    for (const k of keep) m.set(k.toolUseId, k);
  }

  /** All known calls of one agent, oldest first. */
  ticks(sessionId: string, agentId: string): ToolTick[] {
    const key = `${sessionId}/${agentId}`;
    let s = this.sorted.get(key);
    if (!s) {
      const m = this.byAgent.get(key);
      s = m ? [...m.values()].sort((a, b) => a.t - b.t) : [];
      this.sorted.set(key, s);
    }
    return s;
  }
}

/** Calls at or before `t`, oldest first (binary search; `ticks` is sorted). */
export function ticksUntil(ticks: readonly ToolTick[], t: number): readonly ToolTick[] {
  let lo = 0;
  let hi = ticks.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (ticks[mid].t <= t) lo = mid + 1;
    else hi = mid;
  }
  return lo === ticks.length ? ticks : ticks.slice(0, lo);
}

/** Tool name counts, top `n`, the rest summed as "other". */
export function toolBreakdown(ticks: readonly { name: string }[], n = 4): { name: string; n: number }[] {
  const m = new Map<string, number>();
  for (const t of ticks) m.set(t.name, (m.get(t.name) ?? 0) + 1);
  const all = [...m.entries()].map(([name, c]) => ({ name, n: c })).sort((a, b) => b.n - a.n || a.name.localeCompare(b.name));
  if (all.length <= n + 1) return all;
  const rest = all.slice(n).reduce((s, x) => s + x.n, 0);
  return [...all.slice(0, n), { name: 'other', n: rest }];
}
