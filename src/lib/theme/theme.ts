// Theme names, settings persistence and the <html> data attributes (ux-spec 2.8, 4.5, 5.2).
// index.html carries a copy of `bootstrapTheme` as an inline script so the first paint is right.

export const THEMES = ['graphite', 'paper', 'blueprint', 'poster'] as const;
export type ThemeName = (typeof THEMES)[number];
export const THEME_LABELS: Record<ThemeName, string> = {
  graphite: 'Graphite',
  paper: 'Paper',
  blueprint: 'Blueprint',
  poster: 'Poster',
};

export type ModePref = 'system' | 'light' | 'dark';
export type ResolvedMode = 'light' | 'dark';
export type Density = 'comfortable' | 'compact';
export type MotionPref = 'system' | 'full' | 'reduced';
export type TimeAxis = 'elapsed' | 'clock';

export interface RecordingRef {
  path: string;
  name: string;
  startedAt: number | null;
  endedAt: number | null;
  sessionName: string | null;
  openedAt: number;
}

export interface Settings {
  theme: ThemeName;
  mode: ModePref;
  density: Density;
  motion: MotionPref;
  timeAxis: TimeAxis;
  claudeDir: string | null;
  inspectorWidth: number;
  recentRecordings: RecordingRef[];
  /** Unknown keys from storage are kept (2.8). */
  [extra: string]: unknown;
}

export const SETTINGS_KEY = 'virgil.settings.v1';
export const INSPECTOR_MIN = 300;
export const INSPECTOR_MAX = 560;

export const DEFAULT_SETTINGS: Settings = {
  theme: 'graphite',
  mode: 'system',
  density: 'comfortable',
  motion: 'system',
  timeAxis: 'elapsed',
  claudeDir: null,
  inspectorWidth: 360,
  recentRecordings: [],
};

function oneOf<T extends string>(v: unknown, options: readonly T[], fallback: T): T {
  return typeof v === 'string' && (options as readonly string[]).includes(v) ? (v as T) : fallback;
}

/** Parse stored JSON into settings: unknown keys kept, missing or invalid keys take defaults. */
export function parseSettings(raw: string | null | undefined): Settings {
  let obj: Record<string, unknown> = {};
  if (raw) {
    try {
      const v = JSON.parse(raw);
      if (v && typeof v === 'object' && !Array.isArray(v)) obj = v as Record<string, unknown>;
    } catch {
      /* corrupt storage: defaults */
    }
  }
  const width = typeof obj.inspectorWidth === 'number' && Number.isFinite(obj.inspectorWidth) ? obj.inspectorWidth : 360;
  return {
    ...obj,
    theme: oneOf(obj.theme, THEMES, DEFAULT_SETTINGS.theme),
    mode: oneOf(obj.mode, ['system', 'light', 'dark'] as const, 'system'),
    density: oneOf(obj.density, ['comfortable', 'compact'] as const, 'comfortable'),
    motion: oneOf(obj.motion, ['system', 'full', 'reduced'] as const, 'system'),
    timeAxis: oneOf(obj.timeAxis, ['elapsed', 'clock'] as const, 'elapsed'),
    claudeDir: typeof obj.claudeDir === 'string' ? obj.claudeDir : null,
    inspectorWidth: Math.min(INSPECTOR_MAX, Math.max(INSPECTOR_MIN, width)),
    recentRecordings: Array.isArray(obj.recentRecordings)
      ? (obj.recentRecordings.filter((r) => r && typeof r === 'object' && typeof (r as RecordingRef).path === 'string') as RecordingRef[])
      : [],
  };
}

export function resolveMode(mode: ModePref, systemDark: boolean): ResolvedMode {
  return mode === 'system' ? (systemDark ? 'dark' : 'light') : mode;
}

export function resolveMotion(motion: MotionPref, systemReduced: boolean): 'full' | 'reduced' {
  return motion === 'system' ? (systemReduced ? 'reduced' : 'full') : motion;
}

/** The attributes set on <html>. */
export function htmlAttributes(s: Settings, systemDark: boolean, systemReduced: boolean): Record<string, string> {
  return {
    'data-theme': s.theme,
    'data-mode': resolveMode(s.mode, systemDark),
    'data-density': s.density,
    'data-motion': resolveMotion(s.motion, systemReduced),
  };
}
