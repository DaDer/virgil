import { describe, expect, it } from 'vitest';
import { sharedPairs } from '../../data/derive';
import { sixtyAgents } from '../shared/fixtures';
import { sharedLinks } from './sharedLinks';

const pair = (a: string, b: string, files: string[], bothEdited = true) => ({ a, b, files, bothEdited });

describe('sharedLinks', () => {
  const nodes = new Map([
    ['x', 'a:x'],
    ['y', 'a:y'],
    ['z', 'a:z'],
    ['r1', 'r:run'],
    ['r2', 'r:run'],
  ]);

  it('merges agent pairs that land on the same two nodes', () => {
    const res = sharedLinks([pair('x', 'r1', ['/f1']), pair('x', 'r2', ['/f2', '/f1'], false)], nodes, null);
    expect(res.links).toEqual([{ source: 'a:x', target: 'r:run', files: ['/f1', '/f2'], bothEdited: true }]);
  });

  it('drops links inside one node and to agents that are not drawn', () => {
    expect(sharedLinks([pair('r1', 'r2', ['/f']), pair('x', 'gone', ['/f'])], nodes, null).links).toEqual([]);
  });

  it('caps to the heaviest links with nothing selected, shows all of a selected agent', () => {
    const pairs = [pair('x', 'y', ['/1']), pair('y', 'z', ['/1', '/2', '/3']), pair('x', 'z', ['/1', '/2'])];
    const top = sharedLinks(pairs, nodes, null, 2);
    expect(top.total).toBe(3);
    expect(top.links.map((l) => l.files.length)).toEqual([3, 2]);
    const mine = sharedLinks(pairs, nodes, 'x', 1);
    expect(mine.links.map((l) => `${l.source}-${l.target}`)).toEqual(['a:x-a:z', 'a:x-a:y']);
  });

  it('groups real file touches by pair (edits only vs reads and edits)', () => {
    const s = sixtyAgents();
    const touch = (agentId: string, reads: number, writes: number) => ({ agentId, reads, writes, firstAt: 1, lastAt: 2, viaBash: false });
    s.files = {
      '/r/a.ts': { path: '/r/a.ts', touches: { w0: touch('w0', 0, 1), w1: touch('w1', 0, 2), w2: touch('w2', 1, 0) } },
      '/r/b.ts': { path: '/r/b.ts', touches: { w0: touch('w0', 0, 1), w1: touch('w1', 1, 1) } },
    };
    const edits = sharedPairs(s, 'edits');
    expect(edits).toEqual([{ a: 'w0', b: 'w1', files: ['/r/a.ts', '/r/b.ts'], bothEdited: true }]);
    const all = sharedPairs(s, 'all');
    expect(all.map((p) => `${p.a}-${p.b}:${p.files.length}:${p.bothEdited}`)).toEqual(['w0-w1:2:true', 'w0-w2:1:false', 'w1-w2:1:false']);
    const nodesOf = new Map(['w0', 'w1', 'w2'].map((id) => [id, `a:${id}`]));
    expect(sharedLinks(all, nodesOf, null).links).toHaveLength(3);
  });
});
