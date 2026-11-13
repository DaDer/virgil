// The one interface the UI uses to reach data. Two implementations: tauriSource (the Rust core,
// data-contract section 5) and mockSource (a scripted scenario for plain-browser development).

import type { VirgilEvent, EpochMs, PickedFolder, PickedPath, RecordingInfo, Snapshot, SourceStatus, ToolHistory } from '../contract';

export interface SourceHandlers {
  /** A batch of events in seq order (payload of virgil:events). */
  onEvents(events: VirgilEvent[]): void;
  /** A new status (payload of virgil:status). */
  onStatus(status: SourceStatus): void;
}

export interface DataSource {
  readonly kind: 'tauri' | 'mock';
  /** Start listening. Resolves once both channels are subscribed; returns an unsubscribe. */
  connect(handlers: SourceHandlers): Promise<() => void>;
  getSnapshot(): Promise<Snapshot>;
  getStatus(): Promise<SourceStatus>;
  /** Every tool call the source keeps per agent, plus a recording's fs-event times. */
  getToolHistory(): Promise<ToolHistory>;
  listRecordings(dir?: string): Promise<RecordingInfo[]>;
  openRecording(path: string): Promise<SourceStatus>;
  switchToLive(): Promise<SourceStatus>;
  // Replay controls. They reject with CommandError {code: "notReplaying"} in live mode.
  play(): Promise<SourceStatus>;
  pause(): Promise<SourceStatus>;
  seek(t: EpochMs): Promise<SourceStatus>;
  setSpeed(speed: number): Promise<SourceStatus>;
  /** Compress quiet stretches (ux-spec 2.7). Works in any mode; applies to the next recording in live. */
  setSkipGaps(on: boolean): Promise<SourceStatus>;
  /** Optional: a native folder dialog (Tauri only). Resolves with path null when cancelled. */
  pickRecordingFolder?(): Promise<PickedFolder>;
  /** Optional: watch another Claude folder (null: the default). Rejects with notFound for a missing folder. */
  setClaudeDir?(dir: string | null): Promise<SourceStatus>;
  /** Optional: a native folder dialog for the Claude folder. Changes nothing by itself. */
  pickClaudeDir?(): Promise<PickedPath>;
}

export const SPEED_MIN = 0.1;
export const SPEED_MAX = 600;

export function isCommandError(e: unknown): e is { code: string; message: string } {
  return !!e && typeof e === 'object' && 'code' in e && 'message' in e;
}

export function errorText(e: unknown): string {
  if (isCommandError(e)) return e.message;
  if (e instanceof Error) return e.message;
  return String(e);
}

/** True inside the Tauri webview. */
export function inTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}
