// Time to pixel mapping for the Timeline (ux-spec 3.4). Pure.

import { formatClock } from '../../format';

export const MIN_MS_PER_PX = 100;
export const MAX_MS_PER_PX = 60_000;
export const MIN_SPAN = 60_000;
export const TICK_STEPS = [1_000, 5_000, 15_000, 60_000, 300_000, 900_000, 3_600_000, 3 * 3_600_000, 6 * 3_600_000];
export const LABEL_GAP = 80;

export interface Scale {
  /** Time at x = 0. */
  t0: number;
  msPerPx: number;
}

export const toX = (s: Scale, t: number): number => (t - s.t0) / s.msPerPx;
export const toT = (s: Scale, x: number): number => s.t0 + x * s.msPerPx;

export function clampMsPerPx(v: number): number {
  return Math.min(MAX_MS_PER_PX, Math.max(MIN_MS_PER_PX, v));
}

/** Show [from, to] in `width` px; the span is at least MIN_SPAN (a 5 s agent is not full width). */
export function fit(from: number, to: number, width: number, padPx = 24): Scale {
  const span = Math.max(MIN_SPAN, to - from);
  const usable = Math.max(1, width - padPx * 2);
  const msPerPx = clampMsPerPx(span / usable);
  return { t0: from - padPx * msPerPx, msPerPx };
}

/** Zoom by `factor` (>1 zooms out) keeping the time under `anchorX` fixed. */
export function zoomAround(s: Scale, factor: number, anchorX: number): Scale {
  const msPerPx = clampMsPerPx(s.msPerPx * factor);
  const at = toT(s, anchorX);
  return { t0: at - anchorX * msPerPx, msPerPx };
}

export function pan(s: Scale, dx: number): Scale {
  return { t0: s.t0 + dx * s.msPerPx, msPerPx: s.msPerPx };
}

/** A window [a, b] shown in `width` px. */
export function fromWindow(a: number, b: number, width: number): Scale {
  const msPerPx = clampMsPerPx(Math.max(1, b - a) / Math.max(1, width));
  return { t0: a, msPerPx };
}

/** Smallest step from TICK_STEPS whose labels sit at least `minPx` apart. */
export function pickStep(msPerPx: number, minPx = LABEL_GAP): number {
  return TICK_STEPS.find((st) => st / msPerPx >= minPx) ?? TICK_STEPS[TICK_STEPS.length - 1];
}

/** Tick times in [a, b], aligned to multiples of `step` from `origin`. */
export function tickTimes(a: number, b: number, step: number, origin: number): number[] {
  const out: number[] = [];
  let t = origin + Math.ceil((a - origin) / step) * step;
  for (; t <= b && out.length < 500; t += step) out.push(t);
  return out;
}

/** `00:30`, `01:00`, `1:05:00` elapsed since origin; or wall-clock time. */
export function axisLabel(t: number, origin: number, mode: 'elapsed' | 'clock', step: number): string {
  if (mode === 'clock') return formatClock(t, step < 60_000);
  const s = Math.round((t - origin) / 1000);
  const sign = s < 0 ? '-' : '';
  const v = Math.abs(s);
  const h = Math.floor(v / 3600);
  const m = Math.floor(v / 60) % 60;
  const sec = v % 60;
  const p = (n: number) => String(n).padStart(2, '0');
  return h > 0 ? `${sign}${h}:${p(m)}:${p(sec)}` : `${sign}${p(m)}:${p(sec)}`;
}

/** Pixel positions merged: ticks closer than `minGap` px join into one block. */
export function mergeTicks(xs: readonly number[], minGap = 2, minWidth = 1): { x: number; w: number }[] {
  const out: { x: number; w: number }[] = [];
  for (const x of xs) {
    const last = out[out.length - 1];
    if (last && x - (last.x + last.w) < minGap) last.w = Math.max(last.w, x - last.x + minWidth, 2);
    else out.push({ x, w: minWidth });
  }
  return out;
}

/** One SVG path for all ticks of a row, each a rectangle from y to y + h. */
export function tickPath(times: readonly number[], s: Scale, y: number, h: number, width: number): string {
  const xs: number[] = [];
  for (const t of times) {
    const x = toX(s, t);
    if (x >= -2 && x <= width + 2) xs.push(Math.round(x * 2) / 2);
  }
  xs.sort((a, b) => a - b);
  return mergeTicks(xs)
    .map((m) => `M${m.x} ${y}h${m.w}v${h}h${-m.w}z`)
    .join('');
}
