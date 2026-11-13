// Motion budget (ux-spec 5.3, rule 2): at most 12 things loop at once; the most recently
// active running agents keep moving, the rest show the static running style.

import type { Agent } from '../../contract';

export const LOOP_MAX = 12;

export function loopBudget(agents: Iterable<Agent>, max = LOOP_MAX): Set<string> {
  const running: Agent[] = [];
  for (const a of agents) if (a.state === 'running') running.push(a);
  running.sort((a, b) => (b.lastActivityAt ?? b.startedAt ?? 0) - (a.lastActivityAt ?? a.startedAt ?? 0) || a.agentId.localeCompare(b.agentId));
  return new Set(running.slice(0, max).map((a) => a.agentId));
}

/** Whether the replay clock says "do not draw new things yet": an agent starting after `t`. */
export function startsAfter(a: Pick<Agent, 'startedAt' | 'queuedAt'>, t: number): boolean {
  const first = a.queuedAt ?? a.startedAt;
  return first != null && first > t;
}
