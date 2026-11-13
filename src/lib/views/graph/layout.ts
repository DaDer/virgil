// Top-level graph layout with elkjs (ux-spec 3.2): layered, left to right.
// `toElk` and `fromElk` are pure. The engine runs ELK in its own web worker (elk-api plus
// elk-worker), so layout never runs on the main render path; a newer request makes the result
// of an older one be dropped.

import type { ELK, ElkNode } from 'elkjs/lib/elk-api';
import type { LayoutInput } from './buildGraph';

export interface LayoutResult {
  positions: Record<string, { x: number; y: number }>;
  width: number;
  height: number;
}

export const ELK_OPTIONS: Record<string, string> = {
  'elk.algorithm': 'layered',
  'elk.direction': 'RIGHT',
  'elk.hierarchyHandling': 'INCLUDE_CHILDREN',
  'elk.spacing.nodeNode': '16',
  'elk.layered.spacing.nodeNodeBetweenLayers': '56',
  'elk.layered.nodePlacement.strategy': 'BRANDES_KOEPF',
  'elk.layered.considerModelOrder.strategy': 'NODES_AND_EDGES',
  'elk.padding': '[top=0,left=0,bottom=0,right=0]',
};

export function toElk(input: LayoutInput): ElkNode {
  const ids = new Set(input.nodes.map((n) => n.id));
  return {
    id: 'root',
    layoutOptions: ELK_OPTIONS,
    children: input.nodes.map((n) => ({ id: n.id, width: n.width, height: n.height })),
    edges: input.edges.filter((e) => ids.has(e.source) && ids.has(e.target)).map((e) => ({ id: e.id, sources: [e.source], targets: [e.target] })),
  };
}

export function fromElk(res: ElkNode): LayoutResult {
  const positions: LayoutResult['positions'] = {};
  let width = 0;
  let height = 0;
  for (const c of res.children ?? []) {
    const x = Math.round(c.x ?? 0);
    const y = Math.round(c.y ?? 0);
    positions[c.id] = { x, y };
    width = Math.max(width, x + (c.width ?? 0));
    height = Math.max(height, y + (c.height ?? 0));
  }
  return { positions, width, height };
}

/**
 * A simple tree layout used when ELK is unavailable or fails: ranks by distance from the first
 * node, stacked top to bottom within a rank. Same spacing as the ELK options.
 */
export function fallbackLayout(input: LayoutInput): LayoutResult {
  const rank = new Map<string, number>();
  const root = input.nodes[0]?.id;
  if (root) rank.set(root, 0);
  for (let pass = 0; pass < input.nodes.length; pass++) {
    let moved = false;
    for (const e of input.edges) {
      const r = rank.get(e.source);
      if (r != null && (rank.get(e.target) ?? -1) < r + 1 && e.target !== root) {
        if (!rank.has(e.target)) moved = true;
        rank.set(e.target, Math.max(rank.get(e.target) ?? 0, r + 1));
      }
    }
    if (!moved) break;
  }
  const byRank = new Map<number, typeof input.nodes>();
  for (const n of input.nodes) {
    const r = rank.get(n.id) ?? 1;
    const list = byRank.get(r) ?? [];
    list.push(n);
    byRank.set(r, list);
  }
  const positions: LayoutResult['positions'] = {};
  let x = 0;
  let width = 0;
  let height = 0;
  for (const r of [...byRank.keys()].sort((a, b) => a - b)) {
    const list = byRank.get(r)!;
    let y = 0;
    let colW = 0;
    for (const n of list) {
      positions[n.id] = { x, y };
      y += n.height + 16;
      colW = Math.max(colW, n.width);
    }
    height = Math.max(height, y - 16);
    x += colW + 56;
    width = x - 56;
  }
  return { positions, width, height };
}

export class LayoutEngine {
  private elk: Promise<ELK> | null = null;
  private token = 0;

  constructor(private make: () => Promise<ELK>) {}

  private get(): Promise<ELK> {
    return (this.elk ??= this.make());
  }

  /** Lay out `input`. Resolves to null when a newer request was made meanwhile. */
  async request(input: LayoutInput): Promise<LayoutResult | null> {
    const my = ++this.token;
    let result: LayoutResult;
    try {
      const elk = await this.get();
      result = fromElk(await elk.layout(toElk(input)));
    } catch {
      result = fallbackLayout(input);
    }
    return my === this.token ? result : null;
  }

  dispose(): void {
    this.token++;
    void this.elk?.then((e) => e.terminateWorker()).catch(() => {});
    this.elk = null;
  }
}

/** The browser engine: ELK's API on the main thread, the algorithm in elk-worker. */
export function browserLayoutEngine(): LayoutEngine {
  return new LayoutEngine(async () => {
    const [{ default: ELKApi }, { default: workerUrl }] = await Promise.all([
      import('elkjs/lib/elk-api.js'),
      import('elkjs/lib/elk-worker.min.js?url'),
    ]);
    return new ELKApi({ workerUrl });
  });
}
