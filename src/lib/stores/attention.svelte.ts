// The attention list from the shell (src-tauri/src/tower.rs): which sessions need the person,
// finished, went quiet or are working, plus the menu-bar preferences. Tauri only; in a plain
// browser the list stays empty and the panel hides itself.

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { inTauri } from '../data/source';

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

class AttentionStore {
  available = $state(false);
  items = $state<AttentionItem[]>([]);
  prefs = $state<AttentionPrefs>({ keepAwake: true, notifications: true });
  awake = $state(false);
  unseen = $state<string[]>([]);

  groups = $derived(groupAttention(this.items));
  idleCount = $derived(this.items.filter((i) => i.kind === 'idle').length);

  private apply(s: AttentionState): void {
    this.items = s.items;
    this.prefs = s.prefs;
    this.awake = s.awake;
    this.unseen = s.unseen;
  }

  /** Subscribes to the shell. `onFocus` runs when a session is picked from the menu-bar menu. */
  async start(onFocus: (sessionId: string) => void): Promise<void> {
    if (!inTauri()) return;
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
    this.apply(await invoke<AttentionState>('set_attention_prefs', { prefs: { ...this.prefs, ...change } }));
  }

  async markSeen(sessionId: string | null): Promise<void> {
    if (!this.available) return;
    this.apply(await invoke<AttentionState>('mark_attention_seen', { sessionId }));
  }
}

export const attention = new AttentionStore();
