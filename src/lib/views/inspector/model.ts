// Pure helpers for the Inspector (ux-spec 3.5).

import { MAIN_AGENT_ID, type Agent, type Session, type WorkflowRun } from '../../contract';

export interface FileItem {
  path: string;
  mode: 'R' | 'E';
  count: number;
  /** Other agents that touched the same file. */
  others: number;
  viaBash: boolean;
}

/** Files an agent touched: edits first, then reads; most touched first within each. */
export function agentFiles(s: Session, agentId: string): FileItem[] {
  const out: FileItem[] = [];
  for (const f of Object.values(s.files)) {
    const t = f.touches[agentId];
    if (!t) continue;
    out.push({
      path: f.path,
      mode: t.writes > 0 ? 'E' : 'R',
      count: t.reads + t.writes,
      others: Object.keys(f.touches).length - 1,
      viaBash: t.viaBash,
    });
  }
  return out.sort((a, b) => (a.mode === b.mode ? 0 : a.mode === 'E' ? -1 : 1) || b.count - a.count || a.path.localeCompare(b.path));
}

export interface Toucher {
  agent: Agent | null;
  agentId: string;
  mode: 'R' | 'E';
  first: number;
  last: number;
  count: number;
}

/** Who touched a file, earliest first; and how many edited it. */
export function fileTouchers(s: Session, path: string): { touchers: Toucher[]; editors: number } {
  const f = s.files[path];
  if (!f) return { touchers: [], editors: 0 };
  const touchers = Object.values(f.touches)
    .map((t) => ({ agent: s.agents[t.agentId] ?? null, agentId: t.agentId, mode: (t.writes > 0 ? 'E' : 'R') as 'R' | 'E', first: t.firstAt, last: t.lastAt, count: t.reads + t.writes }))
    .sort((a, b) => a.first - b.first || a.agentId.localeCompare(b.agentId));
  return { touchers, editors: touchers.filter((t) => t.mode === 'E').length };
}

/** The largest tokens and tool calls among an agent's peers: its run, else the session. */
export function peerMax(s: Session, a: Agent): { tokens: number; toolCalls: number } {
  const run: WorkflowRun | undefined = a.runId ? s.runs[a.runId] : undefined;
  const peers = run ? run.agentIds.map((id) => s.agents[id]).filter((x): x is Agent => !!x) : Object.values(s.agents).filter((x) => x.agentId !== MAIN_AGENT_ID);
  let tokens = 0;
  let toolCalls = 0;
  for (const p of peers) {
    tokens = Math.max(tokens, p.tokens);
    toolCalls = Math.max(toolCalls, p.toolCalls);
  }
  return { tokens: Math.max(tokens, a.tokens), toolCalls: Math.max(toolCalls, a.toolCalls) };
}

/** A task cut by the core (280 characters, ending with …) is only a preview of the prompt. */
export function isPreview(task: string): boolean {
  return task.length >= 270 && task.endsWith('…');
}
