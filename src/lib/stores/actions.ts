// App-level actions that touch several stores at once (ux-spec 2.6 and 2.9).

import type { RecordingInfo } from '../contract';
import { MAIN_AGENT_ID } from '../contract';
import { errorText } from '../data/source';
import { projectName } from '../format';
import { clock } from './clock.svelte';
import { route, scope, type Route } from './route.svelte';
import { search } from './search.svelte';
import { selection, sel, type Selection } from './selection.svelte';
import { settings } from './settings.svelte';
import { ui } from './ui.svelte';
import { world } from './world.svelte';

export const actions = {
  /** Select something and open the Inspector. */
  select(s: Selection | null): void {
    selection.select(s);
    if (s) ui.inspectorOpen = true;
  },

  /** Scope to a session and go to a view (default Graph). */
  openSession(sessionId: string, to: Route = 'graph'): void {
    scope.openSession(sessionId);
    route.navigate(to);
  },

  /** `P`: select the parent of the selected agent. */
  selectParent(): void {
    const cur = selection.current;
    if (!cur || cur.kind !== 'agent' || !cur.sessionId) return;
    const a = world.agent(cur.sessionId, cur.id);
    if (!a) return;
    if (a.runId && a.phaseIndex != null) return actions.select(sel.phase(cur.sessionId, a.runId, a.phaseIndex));
    if (a.agentId === MAIN_AGENT_ID) return actions.select(sel.session(cur.sessionId));
    const parent = a.parentAgentId ?? MAIN_AGENT_ID;
    if (parent === MAIN_AGENT_ID) return actions.select(sel.session(cur.sessionId));
    actions.select(sel.agent(cur.sessionId, parent));
  },

  /** Esc: close a dialog, else clear search, else clear selection. */
  escape(): void {
    if (ui.closeDialogs()) return;
    if (search.active) return search.clear();
    selection.clear();
  },

  /** The Replay segment: open the recording picker. */
  chooseReplay(): void {
    ui.pickerError = null;
    ui.pickerOpen = true;
  },

  async openRecording(path: string, info?: RecordingInfo): Promise<boolean> {
    const source = world.source;
    if (!source) return false;
    try {
      ui.pickerError = null;
      if (world.mode === 'live') ui.liveScope = { ...scope.value };
      const status = await source.openRecording(path);
      world.setStatus(status);
      selection.clear();
      const rec = status.recording ?? info ?? null;
      const sessionId = rec?.sessionId ?? null;
      scope.openSession(sessionId);
      if (sessionId) route.navigate('graph');
      ui.pickerOpen = false;
      ui.replayHint = true;
      if (rec) {
        settings.rememberRecording({
          path: rec.path,
          name: rec.name,
          startedAt: rec.startedAt,
          endedAt: rec.endedAt,
          sessionName: rec.projectSlug ? projectName(rec.projectSlug.replace(/^-/, '/').replace(/-/g, '/')) : null,
        });
      }
      return true;
    } catch (e) {
      ui.pickerError = errorText(e);
      return false;
    }
  },

  async goLive(): Promise<void> {
    const source = world.source;
    if (!source || world.mode === 'live') return;
    try {
      const status = await source.switchToLive();
      world.setStatus(status);
      selection.clear();
      ui.replayHint = false;
      const prev = ui.liveScope;
      scope.set(prev ?? { sessionId: null, runId: null });
      if (!prev?.sessionId) route.navigate('overview');
    } catch (e) {
      clock.lastError = errorText(e);
    }
  },

  /** Settings, StatusBar and the missing-folder state: pick the Claude folder and watch it. */
  async chooseClaudeDir(): Promise<void> {
    ui.folderError = null;
    if (!world.canChooseClaudeDir) {
      ui.settingsOpen = true;
      return;
    }
    try {
      const dir = await world.pickClaudeDir();
      if (!dir) return;
      await actions.useClaudeDir(dir);
    } catch (e) {
      ui.folderError = errorText(e);
      ui.settingsOpen = true;
    }
  },

  /** Watch `dir` (null: auto-detect) and remember the choice. */
  async useClaudeDir(dir: string | null): Promise<void> {
    ui.folderError = null;
    try {
      await world.setClaudeDir(dir);
      settings.update({ claudeDir: dir });
      if (world.mode === 'live') {
        selection.clear();
        scope.set({ sessionId: null, runId: null });
        route.navigate('overview');
      }
    } catch (e) {
      ui.folderError = errorText(e);
      ui.settingsOpen = true;
    }
  },

  /** At start-up: send the remembered Claude folder to the core. A folder that is gone is
   *  forgotten, and the error shows in Settings. */
  async applyStoredClaudeDir(): Promise<void> {
    const dir = settings.value.claudeDir;
    if (!dir || !world.source?.setClaudeDir || world.status?.claudeDir === dir) return;
    try {
      await world.setClaudeDir(dir);
    } catch (e) {
      ui.folderError = `${errorText(e)}. Using the default folder.`;
      settings.update({ claudeDir: null });
    }
  },

  async togglePlay(): Promise<void> {
    ui.replayHint = false;
    await clock.toggle();
  },
};
