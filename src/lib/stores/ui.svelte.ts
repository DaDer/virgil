// Frame-level UI state: open dialogs, Inspector visibility, and the replay hint.

class UiStore {
  settingsOpen = $state(false);
  pickerOpen = $state(false);
  shortcutsOpen = $state(false);
  problemsOpen = $state(false);
  inspectorOpen = $state(false);
  /** Shown after a recording loads, until the first play. */
  replayHint = $state(false);
  /** Error shown inside the recording picker. */
  pickerError = $state<string | null>(null);
  /** Error from choosing the Claude folder, shown in Settings. */
  folderError = $state<string | null>(null);
  /** The live scope to restore when returning from replay. */
  liveScope: { sessionId: string | null; runId: string | null } | null = null;

  get anyDialog(): boolean {
    return this.settingsOpen || this.pickerOpen || this.shortcutsOpen || this.problemsOpen;
  }

  closeDialogs(): boolean {
    const had = this.anyDialog;
    this.settingsOpen = this.pickerOpen = this.shortcutsOpen = this.problemsOpen = false;
    return had;
  }
}

export const ui = new UiStore();
