// Search (ux-spec 2.3): one text, one match set computed once and read by every view.
// Keys in `matches`: a sessionId for sessions, `${sessionId}/${agentId}` for agents.
// Non-matching things are dimmed to 25%, never removed.

import type { Agent, Session } from '../contract';
import { route, scope } from './route.svelte';
import { world } from './world.svelte';

export function agentKey(sessionId: string, agentId: string): string {
  return `${sessionId}/${agentId}`;
}

function agentHaystack(s: Session, a: Agent, files: Map<string, { path: string }[]>): string {
  const paths = (files.get(a.agentId) ?? []).map((f) => f.path).join(' ');
  return `${a.label} ${a.task} ${a.agentType ?? ''} ${a.model ?? ''} ${a.name ?? ''} ${paths}`.toLowerCase();
}

function sessionHaystack(s: Session): string {
  return `${s.name ?? ''} ${s.title ?? ''} ${s.cwd ?? ''} ${s.sessionId}`.toLowerCase();
}

class SearchStore {
  text = $state('');
  /** Set by the TopBar so `/` and Mod+K can focus it. */
  input: HTMLInputElement | null = null;

  get active(): boolean {
    return this.text.trim().length > 0;
  }

  /** The match set. Empty when no search is active: check `active` or use `isMatch`. */
  matches = $derived.by(() => {
    const q = this.text.trim().toLowerCase();
    const out = new Set<string>();
    if (!q) return out;
    const terms = q.split(/\s+/);
    const hit = (h: string) => terms.every((t) => h.includes(t));
    for (const s of Object.values(world.snapshot.sessions)) {
      const files = world.filesByAgent(s);
      let any = false;
      for (const a of Object.values(s.agents)) {
        if (hit(agentHaystack(s, a, files))) {
          out.add(agentKey(s.sessionId, a.agentId));
          any = true;
        }
      }
      if (any || hit(sessionHaystack(s))) out.add(s.sessionId);
    }
    return out;
  });

  /** True when nothing is searched or the thing matches. */
  isMatch(sessionId: string, agentId?: string): boolean {
    if (!this.active) return true;
    return this.matches.has(agentId === undefined ? sessionId : agentKey(sessionId, agentId));
  }

  /** "7 of 58": agents of the scoped session in views, sessions on Overview. */
  count = $derived.by(() => {
    if (!this.active) return undefined;
    if (route.current === 'overview' || !scope.sessionId) {
      const all = Object.keys(world.snapshot.sessions);
      return { shown: all.filter((id) => this.matches.has(id)).length, total: all.length };
    }
    const s = world.session(scope.sessionId);
    const ids = s ? Object.keys(s.agents) : [];
    return { shown: ids.filter((id) => this.matches.has(agentKey(scope.sessionId!, id))).length, total: ids.length };
  });

  clear(): void {
    this.text = '';
  }

  focus(): void {
    this.input?.focus();
    this.input?.select();
  }
}

export const search = new SearchStore();
