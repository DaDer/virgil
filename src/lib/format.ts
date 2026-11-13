// Number, time, path and model formats (ux-spec 1.3 and 1.5). Pure functions.

/** `950`, `41k`, `375k`, `5.2M`, `12M`. One decimal only below 10. */
export function formatCount(n: number | null | undefined): string {
  if (n == null || !Number.isFinite(n)) return '–';
  const abs = Math.abs(n);
  const sign = n < 0 ? '-' : '';
  const unit = (v: number, u: string) => {
    const r = v < 10 ? Math.round(v * 10) / 10 : Math.round(v);
    return `${sign}${r}${u}`;
  };
  if (abs < 1000) return `${Math.round(n)}`;
  if (abs < 999_500) return unit(abs / 1000, 'k');
  if (abs < 999_500_000) return unit(abs / 1_000_000, 'M');
  return unit(abs / 1_000_000_000, 'B');
}

/** `1,636`: full number with grouping, for totals. */
export function formatInt(n: number | null | undefined): string {
  if (n == null || !Number.isFinite(n)) return '–';
  return Math.round(n).toLocaleString('en-US');
}

/** `420 ms` (only with allowMs), `42s`, `17m 20s`, `9h 06m`. */
export function formatDuration(ms: number | null | undefined, opts: { allowMs?: boolean } = {}): string {
  if (ms == null || !Number.isFinite(ms) || ms < 0) return '–';
  if (opts.allowMs && ms < 1000) return `${Math.round(ms)} ms`;
  const s = Math.floor(ms / 1000);
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ${String(s % 60).padStart(2, '0')}s`;
  const h = Math.floor(m / 60);
  return `${h}h ${String(m % 60).padStart(2, '0')}m`;
}

/** `01:12:40`: elapsed as a clock readout. */
export function formatElapsed(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  const p = (n: number) => String(n).padStart(2, '0');
  return `${p(Math.floor(s / 3600))}:${p(Math.floor(s / 60) % 60)}:${p(s % 60)}`;
}

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

/** `just now`, `35s ago`, `12m ago`, `3h ago`, then `12 Nov, 14:05`. */
export function formatRelative(t: number | null | undefined, now: number): string {
  if (t == null || !Number.isFinite(t)) return '–';
  const d = now - t;
  if (d < 10_000) return 'just now';
  if (d < 60_000) return `${Math.floor(d / 1000)}s ago`;
  if (d < 3_600_000) return `${Math.floor(d / 60_000)}m ago`;
  if (d < 24 * 3_600_000) return `${Math.floor(d / 3_600_000)}h ago`;
  return formatDate(t);
}

export function formatDate(t: number): string {
  const d = new Date(t);
  return `${d.getDate()} ${MONTHS[d.getMonth()]}, ${formatClock(t, false)}`;
}

/** Clock time in the system locale (24 h or 12 h by system setting). */
export function formatClock(t: number | null | undefined, seconds = true): string {
  if (t == null || !Number.isFinite(t)) return '–';
  return new Date(t).toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit', ...(seconds ? { second: '2-digit' } : {}) });
}

// ---------- models ----------

export type ModelFamily = 'opus' | 'sonnet' | 'haiku' | 'fable' | 'other';

export function modelFamily(model: string | null | undefined): ModelFamily {
  const m = (model ?? '').toLowerCase();
  for (const f of ['opus', 'sonnet', 'haiku', 'fable'] as const) if (m.includes(f)) return f;
  return 'other';
}

export const MODEL_LETTER: Record<ModelFamily, string> = { opus: 'O', sonnet: 'S', haiku: 'H', fable: 'F', other: '?' };

/** `claude-opus-5-5` → `Opus 5.5`; drops 8-digit dates and bracket suffixes. */
export function shortModel(model: string | null | undefined): string {
  if (!model) return 'Unknown model';
  let m = model.trim().replace(/\[[^\]]*\]$/, '');
  m = m.replace(/^claude-/, '');
  const parts = m.split(/[-_]/).filter((p) => p && !/^\d{8}$/.test(p));
  const fam = parts.findIndex((p) => /^(opus|sonnet|haiku|fable)$/i.test(p));
  if (fam < 0) return model;
  const name = parts[fam][0].toUpperCase() + parts[fam].slice(1).toLowerCase();
  const nums = parts.filter((p, i) => i !== fam && /^\d+$/.test(p));
  return nums.length ? `${name} ${nums.join('.')}` : name;
}

// ---------- paths ----------

/** Relative to cwd when inside it; else `~` for a home dir; forward slashes. */
export function displayPath(path: string, cwd?: string | null): string {
  const p = path.replace(/\\/g, '/');
  if (cwd) {
    const c = cwd.replace(/\\/g, '/').replace(/\/$/, '');
    if (p === c) return '.';
    if (p.startsWith(c + '/')) return p.slice(c.length + 1);
  }
  const home = p.match(/^(\/Users\/[^/]+|\/home\/[^/]+|[A-Za-z]:\/Users\/[^/]+)(\/|$)/);
  if (home) return '~' + p.slice(home[1].length);
  return p;
}

/** `src/…/Card.svelte`: keep the first part and the file name, drop the middle. */
export function middleTruncate(path: string, max = 48): string {
  if (path.length <= max) return path;
  const parts = path.split('/');
  const last = parts[parts.length - 1];
  if (parts.length > 2) {
    const head = parts[0] === '' ? '/' + parts[1] : parts[0];
    const candidate = `${head}/…/${last}`;
    if (candidate.length <= max) {
      // add back parent folders while they fit
      let tail = last;
      for (let i = parts.length - 2; i > 1; i--) {
        const next = `${parts[i]}/${tail}`;
        if (`${head}/…/${next}`.length > max) break;
        tail = next;
      }
      return `${head}/…/${tail}`;
    }
  }
  const keep = max - 1;
  return path.slice(0, Math.ceil(keep / 2)) + '…' + path.slice(path.length - Math.floor(keep / 2));
}

/** Last two parts of a cwd: `pixel/virgil`. */
export function projectName(cwd: string | null | undefined): string {
  if (!cwd) return '–';
  const parts = cwd.replace(/\\/g, '/').split('/').filter(Boolean);
  return parts.slice(-2).join('/') || cwd;
}

export function basename(path: string): string {
  const parts = path.replace(/\\/g, '/').split('/');
  return parts[parts.length - 1] || path;
}

/** First non-empty line of a task, cut at `max` characters with an ellipsis. */
export function taskSummary(task: string | null | undefined, max = 120): string {
  const line = (task ?? '').split('\n').find((l) => l.trim()) ?? '';
  const t = line.trim();
  return t.length > max ? t.slice(0, max - 1).trimEnd() + '…' : t;
}
