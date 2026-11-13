// DataSource backed by the Rust core through Tauri commands and event channels
// (data-contract section 5). Command and channel names come from the contract.

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  EVENT_CHANNEL,
  STATUS_CHANNEL,
  type Commands,
  type EventBatch,
  type SourceStatus,
} from '../contract';
import type { DataSource } from './source';

// Diagnostics: the webview reports what it receives to the shell, which prints it to stderr.
// Run the app binary from a terminal to read it.
const diag = { batches: 0, events: 0, lastSeq: 0, statuses: 0 };
function report(line: string): void {
  void invoke('ui_report', { line }).catch(() => {});
}

async function call<K extends keyof Commands>(cmd: K, args?: Commands[K]['args']): Promise<Commands[K]['returns']> {
  const started = performance.now();
  try {
    const out = await invoke<Commands[K]['returns']>(cmd, args as Record<string, unknown> | undefined);
    const ms = Math.round(performance.now() - started);
    if (cmd === 'get_snapshot') {
      const snap = out as { seq?: number; sessions?: Record<string, unknown> };
      report(`get_snapshot ok in ${ms} ms: seq ${snap.seq}, sessions ${Object.keys(snap.sessions ?? {}).length}`);
    } else if (ms > 500) {
      report(`${cmd} ok in ${ms} ms`);
    }
    return out;
  } catch (e) {
    report(`${cmd} FAILED: ${typeof e === 'string' ? e : JSON.stringify(e)}`);
    throw e;
  }
}

export function createTauriSource(): DataSource {
  return {
    kind: 'tauri',
    async connect(h) {
      window.addEventListener('error', (e) => report(`window error: ${e.message} at ${e.filename}:${e.lineno}`));
      window.addEventListener('unhandledrejection', (e) => report(`unhandled rejection: ${String(e.reason)}`));
      const offEvents = await listen<EventBatch>(EVENT_CHANNEL, (e) => {
        diag.batches += 1;
        diag.events += e.payload.events.length;
        diag.lastSeq = e.payload.events.at(-1)?.seq ?? diag.lastSeq;
        try {
          h.onEvents(e.payload.events);
        } catch (err) {
          report(`onEvents threw: ${String(err)}`);
          throw err;
        }
      });
      const offStatus = await listen<SourceStatus>(STATUS_CHANNEL, (e) => {
        diag.statuses += 1;
        h.onStatus(e.payload);
      });
      report('connected: listening on both channels');
      const timer = setInterval(
        () => {
          report(`received ${diag.batches} batches, ${diag.events} events, last seq ${diag.lastSeq}, ${diag.statuses} status updates`);
          const store = (globalThis as { __virgilDiag?: () => string }).__virgilDiag;
          if (store) report(store());
        },
        5000,
      );
      return () => {
        clearInterval(timer);
        offEvents();
        offStatus();
      };
    },
    getSnapshot: () => call('get_snapshot'),
    getStatus: () => call('get_status'),
    listRecordings: (dir) => call('list_recordings', dir === undefined ? {} : { dir }),
    openRecording: (path) => call('open_recording', { path }),
    switchToLive: () => call('switch_to_live'),
    play: () => call('replay_play'),
    pause: () => call('replay_pause'),
    seek: (t) => call('replay_seek', { t: Math.round(t) }),
    setSpeed: (speed) => call('replay_set_speed', { speed }),
    pickRecordingFolder: () => call('pick_recording_folder'),
    getToolHistory: () => call('get_tool_history'),
    setSkipGaps: (on) => call('set_skip_gaps', { on }),
    setClaudeDir: (dir) => call('set_claude_dir', { dir }),
    pickClaudeDir: () => call('pick_claude_dir'),
  };
}
