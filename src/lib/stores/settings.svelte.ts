// Settings store (ux-spec 2.8): persisted to localStorage, applied as data-* attributes on <html>.

import {
  DEFAULT_SETTINGS,
  INSPECTOR_MAX,
  INSPECTOR_MIN,
  SETTINGS_KEY,
  htmlAttributes,
  parseSettings,
  type RecordingRef,
  type Settings,
} from '../theme/theme';

const RECENT_MAX = 12;

function storage(): Storage | null {
  try {
    return typeof localStorage === 'undefined' ? null : localStorage;
  } catch {
    return null;
  }
}

function media(q: string): MediaQueryList | null {
  return typeof window !== 'undefined' && window.matchMedia ? window.matchMedia(q) : null;
}

class SettingsStore {
  value = $state<Settings>({ ...DEFAULT_SETTINGS });
  systemDark = $state(false);
  systemReduced = $state(false);
  private started = false;
  private switchTimer: ReturnType<typeof setTimeout> | null = null;

  /** Resolved light or dark. */
  get resolvedMode(): 'light' | 'dark' {
    return this.value.mode === 'system' ? (this.systemDark ? 'dark' : 'light') : this.value.mode;
  }

  get reducedMotion(): boolean {
    return this.value.motion === 'reduced' || (this.value.motion === 'system' && this.systemReduced);
  }

  init(): void {
    if (this.started) return;
    this.started = true;
    this.value = parseSettings(storage()?.getItem(SETTINGS_KEY));
    const dark = media('(prefers-color-scheme: dark)');
    const reduced = media('(prefers-reduced-motion: reduce)');
    this.systemDark = !!dark?.matches;
    this.systemReduced = !!reduced?.matches;
    dark?.addEventListener('change', (e) => {
      this.systemDark = e.matches;
      this.apply(true);
    });
    reduced?.addEventListener('change', (e) => {
      this.systemReduced = e.matches;
      this.apply(false);
    });
    this.apply(false);
  }

  update(patch: Partial<Settings>): void {
    const prev = this.value;
    this.value = { ...prev, ...patch };
    if (this.value.inspectorWidth !== prev.inspectorWidth) {
      this.value.inspectorWidth = Math.round(Math.min(INSPECTOR_MAX, Math.max(INSPECTOR_MIN, this.value.inspectorWidth)));
    }
    this.save();
    this.apply(prev.theme !== this.value.theme || prev.mode !== this.value.mode);
  }

  rememberRecording(ref: Omit<RecordingRef, 'openedAt'>): void {
    const rest = this.value.recentRecordings.filter((r) => r.path !== ref.path);
    this.update({ recentRecordings: [{ ...ref, openedAt: Date.now() }, ...rest].slice(0, RECENT_MAX) });
  }

  private save(): void {
    try {
      storage()?.setItem(SETTINGS_KEY, JSON.stringify(this.value));
    } catch {
      /* storage full or blocked: settings stay in memory */
    }
  }

  private apply(crossFade: boolean): void {
    if (typeof document === 'undefined') return;
    const el = document.documentElement;
    const attrs = htmlAttributes(this.value, this.systemDark, this.systemReduced);
    const changed = attrs['data-theme'] !== el.dataset.theme || attrs['data-mode'] !== el.dataset.mode;
    if (crossFade && changed && attrs['data-motion'] !== 'reduced') {
      el.classList.add('theme-switching');
      if (this.switchTimer) clearTimeout(this.switchTimer);
      this.switchTimer = setTimeout(() => el.classList.remove('theme-switching'), 300);
    }
    for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, v);
  }
}

export const settings = new SettingsStore();
