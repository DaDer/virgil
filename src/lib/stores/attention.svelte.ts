// The attention list: which sessions need the person, finished, went quiet or are working, plus
// the menu-bar preferences. In the app it comes from the shell (src-tauri/src/tower.rs). In a plain
// browser it is derived from the mock scenario, so the Now panel can be worked on and demonstrated.

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Session, Snapshot } from '../contract';
import { DEMO_LAST_MESSAGE } from '../data/scenario';
import { inTauri } from '../data/source';
import { clock } from './clock.svelte';
import { world } from './world.svelte';

export type AttentionKind = 'needsYou' | 'finished' | 'quiet' | 'working' | 'idle';

export interface AttentionItem {
  sessionId: string;
  name: string;
  project: string | null;
  title: string | null;
  kind: AttentionKind;
  /** When it entered this state (epoch ms). */
  since: number;
  /** Where it stands, in one line. */
  line: string;
  runningAgents: number;
  backgroundTasks: number;
}

export interface AttentionPrefs {
  keepAwake: boolean;
  notifications: boolean;
}

export interface AttentionState {
  items: AttentionItem[];
  prefs: AttentionPrefs;
  /** Whether the wake lock is held right now. */
  awake: boolean;
  unseen: string[];
}

export const ATTENTION_CHANNEL = 'virgil:attention';
export const FOCUS_CHANNEL = 'virgil:focus-session';

export const KIND_LABEL: Record<AttentionKind, string> = {
  needsYou: 'Needs you',
  finished: 'Finished',
  quiet: 'Gone quiet',
  working: 'Working',
  idle: 'Idle',
};

/** Groups in display order, without idle sessions and without empty groups. */
export function groupAttention(items: AttentionItem[]): { kind: AttentionKind; items: AttentionItem[] }[] {
  const order: AttentionKind[] = ['needsYou', 'finished', 'quiet', 'working'];
  return order.map((kind) => ({ kind, items: items.filter((i) => i.kind === kind) })).filter((g) => g.items.length > 0);
}

const QUIET_AFTER_MS = 15 * 60_000;
const MAIN = 'main';

/**
 * The attention rules for the mock scenario, a simplified copy of `crates/core/src/attention.rs`
 * (no background commands, which the mock does not have). `lines` gives each session's last message.
 */
export function deriveDemoAttention(snapshot: Snapshot, now: number, lines: Record<string, string> = {}): AttentionItem[] {
  const order: AttentionKind[] = ['needsYou', 'finished', 'quiet', 'working', 'idle'];
  const one = (s: Session): AttentionItem => {
    const agents = Object.values(s.agents).filter((a) => a.agentId !== MAIN && !a.superseded);
    const running = agents.filter((a) => a.state === 'running' || a.state === 'queued');
    const statusAt = s.statusUpdatedAt ?? s.updatedAt ?? s.startedAt ?? now;
    const lastActivity = Math.max(statusAt, ...Object.values(s.agents).map((a) => a.lastActivityAt ?? 0));
    const said = lines[s.sessionId];
    const open = [...(s.agents[MAIN]?.recentTools ?? [])].reverse().find((c) => c.endedAt == null);
    let kind: AttentionKind;
    let since = statusAt;
    let line: string;
    if (s.status === 'waiting') {
      kind = 'needsYou';
      line = said ?? 'Waiting for your answer';
    } else if (s.status === 'busy' || s.status === 'shell' || s.status == null) {
      line = running.length
        ? running.length === 1
          ? `1 agent running: ${running[0].label}`
          : `${running.length} agents running: ${running[0].label} and ${running.length - 1} more`
        : open
          ? open.summary
            ? `${open.name}: ${open.summary}`
            : `Running ${open.name}`
          : 'Thinking';
      kind = now - lastActivity > QUIET_AFTER_MS ? 'quiet' : 'working';
      if (kind === 'quiet') since = lastActivity;
    } else if (running.length) {
      kind = 'working';
      line = `Waiting for ${running.length} agent${running.length === 1 ? '' : 's'}: ${running[0].label}`;
    } else {
      kind = 'finished';
      line = said ?? 'Finished';
    }
    const project = s.cwd?.split(/[\\/]/).filter(Boolean).pop() ?? null;
    return {
      sessionId: s.sessionId,
      name: s.name ?? s.title ?? project ?? s.sessionId.slice(0, 8),
      project,
      title: s.title,
      kind,
      since,
      line,
      runningAgents: running.length,
      backgroundTasks: 0,
    };
  };
  return Object.values(snapshot.sessions)
    .filter((s) => s.liveness !== 'stale')
    .map(one)
    .sort((a, b) => order.indexOf(a.kind) - order.indexOf(b.kind) || b.since - a.since);
}

class AttentionStore {
  available = $state(false);
  /** True in a plain browser: the list is derived from the mock scenario. */
  demo = $state(false);
  private liveItems = $state<AttentionItem[]>([]);
  prefs = $state<AttentionPrefs>({ keepAwake: true, notifications: true });
  private liveAwake = $state(false);
  unseen = $state<string[]>([]);

  items = $derived(this.demo ? deriveDemoAttention(world.snapshot, clock.viewTime, DEMO_LAST_MESSAGE) : this.liveItems);
  awake = $derived(this.demo ? this.prefs.keepAwake && this.items.some((i) => i.kind === 'working' || i.kind === 'quiet') : this.liveAwake);
  groups = $derived(groupAttention(this.items));
  idleCount = $derived(this.items.filter((i) => i.kind === 'idle').length);

  private apply(s: AttentionState): void {
    this.liveItems = s.items;
    this.prefs = s.prefs;
    this.liveAwake = s.awake;
    this.unseen = s.unseen;
  }

  /** Subscribes to the shell. `onFocus` runs when a session is picked from the menu-bar menu. */
  async start(onFocus: (sessionId: string) => void): Promise<void> {
    if (!inTauri()) {
      this.demo = true;
      this.available = true;
      return;
    }
    try {
      await listen<AttentionState>(ATTENTION_CHANNEL, (e) => this.apply(e.payload));
      await listen<string>(FOCUS_CHANNEL, (e) => onFocus(e.payload));
      this.apply(await invoke<AttentionState>('get_attention'));
      this.available = true;
    } catch {
      this.available = false;
    }
  }

  async setPrefs(change: Partial<AttentionPrefs>): Promise<void> {
    if (this.demo) {
      this.prefs = { ...this.prefs, ...change };
      return;
    }
    this.apply(await invoke<AttentionState>('set_attention_prefs', { prefs: { ...this.prefs, ...change } }));
  }

  async markSeen(sessionId: string | null): Promise<void> {
    if (!this.available || this.demo) return;
    this.apply(await invoke<AttentionState>('mark_attention_seen', { sessionId }));
  }
}

export const attention = new AttentionStore();
