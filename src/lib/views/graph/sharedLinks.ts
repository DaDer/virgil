// Shared-file links for the Graph overlay (ux-spec 3.2): one link per pair of drawn nodes,
// however many files they share. Pure.

import type { SharedPair } from '../../data/derive';

export interface SharedLink {
  source: string;
  target: string;
  files: string[];
  bothEdited: boolean;
}

/**
 * Map agent pairs to node pairs (agents of a collapsed run map to the run box), merge pairs that
 * land on the same two nodes, and pick what to draw: all links of `focusAgent` when one is
 * selected, else the `max` heaviest.
 */
export function sharedLinks(
  pairs: readonly SharedPair[],
  agentNode: ReadonlyMap<string, string>,
  focusAgent: string | null,
  max = 40,
): { links: SharedLink[]; total: number } {
  const merged = new Map<string, SharedLink & { agents: Set<string> }>();
  for (const p of pairs) {
    const a = agentNode.get(p.a);
    const b = agentNode.get(p.b);
    if (!a || !b || a === b) continue;
    const [source, target] = a < b ? [a, b] : [b, a];
    const key = `${source}\u0000${target}`;
    let l = merged.get(key);
    if (!l) merged.set(key, (l = { source, target, files: [], bothEdited: false, agents: new Set() }));
    for (const f of p.files) if (!l.files.includes(f)) l.files.push(f);
    l.bothEdited ||= p.bothEdited;
    l.agents.add(p.a).add(p.b);
  }
  let all = [...merged.values()];
  const total = all.length;
  if (focusAgent) all = all.filter((l) => l.agents.has(focusAgent));
  all.sort((x, y) => y.files.length - x.files.length || (x.source + x.target).localeCompare(y.source + y.target));
  if (!focusAgent) all = all.slice(0, max);
  return { links: all.map(({ agents: _a, ...l }) => ({ ...l, files: [...l.files].sort() })), total };
}
