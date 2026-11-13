import { describe, expect, it } from 'vitest';
import { sixtyAgents } from '../shared/fixtures';
import { agentFiles, fileTouchers, isPreview, peerMax } from './model';

const touch = (agentId: string, reads: number, writes: number, firstAt = 1, lastAt = 2) => ({ agentId, reads, writes, firstAt, lastAt, viaBash: false });

function withFiles() {
  const s = sixtyAgents();
  s.files = {
    '/r/a.ts': { path: '/r/a.ts', touches: { w0: touch('w0', 2, 0, 5), w1: touch('w1', 0, 1, 3) } },
    '/r/b.ts': { path: '/r/b.ts', touches: { w0: touch('w0', 0, 3) } },
    '/r/c.ts': { path: '/r/c.ts', touches: { w0: touch('w0', 1, 0), w1: touch('w1', 0, 2), w2: touch('w2', 0, 1) } },
  };
  return s;
}

describe('inspector model', () => {
  it('lists an agent’s files, edits first, with other touchers', () => {
    const items = agentFiles(withFiles(), 'w0');
    expect(items.map((f) => `${f.mode}:${f.path}:${f.count}:+${f.others}`)).toEqual(['E:/r/b.ts:3:+0', 'R:/r/a.ts:2:+1', 'R:/r/c.ts:1:+2']);
    expect(agentFiles(withFiles(), 'nobody')).toEqual([]);
  });

  it('lists who touched a file and counts editors', () => {
    const { touchers, editors } = fileTouchers(withFiles(), '/r/c.ts');
    expect(touchers.map((t) => `${t.agentId}:${t.mode}`)).toEqual(['w0:R', 'w1:E', 'w2:E']);
    expect(editors).toBe(2);
    expect(fileTouchers(withFiles(), '/missing').touchers).toEqual([]);
  });

  it('compares an agent with the largest in its run', () => {
    const s = sixtyAgents();
    expect(peerMax(s, s.agents.w0)).toEqual({ tokens: 40_000 + 57 * 1000, toolCalls: 19 });
  });

  it('marks a cut task as a preview', () => {
    expect(isPreview('x'.repeat(279) + '…')).toBe(true);
    expect(isPreview('short task')).toBe(false);
  });
});
