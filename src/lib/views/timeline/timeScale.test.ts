import { describe, expect, it } from 'vitest';
import { axisLabel, clampMsPerPx, fit, fromWindow, mergeTicks, pan, pickStep, tickPath, tickTimes, toT, toX, zoomAround } from './timeScale';

describe('timeScale', () => {
  it('maps time to pixels and back', () => {
    const s = { t0: 1000, msPerPx: 100 };
    expect(toX(s, 2000)).toBe(10);
    expect(toT(s, 10)).toBe(2000);
    expect(pan(s, 5).t0).toBe(1500);
  });

  it('fits a range with padding and a 60 s minimum span', () => {
    const s = fit(0, 600_000, 1048, 24);
    expect(s.msPerPx).toBeCloseTo(600);
    expect(toX(s, 0)).toBeCloseTo(24);
    expect(toX(s, 600_000)).toBeCloseTo(1024);
    const short = fit(0, 5_000, 1048, 24);
    // 60 s over 1000 px is 60 ms/px, clamped to the 100 ms/px limit.
    expect(short.msPerPx).toBe(100);
  });

  it('clamps zoom from 100 ms to 1 min per pixel', () => {
    expect(clampMsPerPx(5)).toBe(100);
    expect(clampMsPerPx(1e9)).toBe(60_000);
    expect(fit(0, 9 * 3_600_000 + 6 * 60_000, 1000, 0).msPerPx).toBeCloseTo(32_760);
  });

  it('zooms around the pointer', () => {
    const s = { t0: 0, msPerPx: 1000 };
    const z = zoomAround(s, 0.5, 200);
    expect(z.msPerPx).toBe(500);
    expect(toT(z, 200)).toBe(toT(s, 200));
    expect(fromWindow(0, 100_000, 1000).msPerPx).toBe(100);
  });

  it('picks tick steps so labels sit at least 80 px apart', () => {
    expect(pickStep(100)).toBe(15_000);
    expect(pickStep(1000)).toBe(300_000);
    expect(pickStep(12)).toBe(1_000);
    expect(pickStep(60_000)).toBe(3 * 3_600_000);
  });

  it('aligns ticks to the origin', () => {
    expect(tickTimes(1_500, 5_000, 1_000, 500)).toEqual([1_500, 2_500, 3_500, 4_500]);
    expect(tickTimes(0, 10, 100, 50)).toEqual([]);
  });

  it('labels elapsed time', () => {
    expect(axisLabel(90_000, 0, 'elapsed', 15_000)).toBe('01:30');
    expect(axisLabel(3_725_000, 0, 'elapsed', 60_000)).toBe('1:02:05');
    expect(axisLabel(-5_000, 0, 'elapsed', 1_000)).toBe('-00:05');
  });

  it('merges ticks closer than 2 px', () => {
    expect(mergeTicks([0, 1, 1.5, 10, 30, 31])).toEqual([
      { x: 0, w: 2.5 },
      { x: 10, w: 1 },
      { x: 30, w: 2 },
    ]);
    expect(mergeTicks([])).toEqual([]);
  });

  it('builds one path for a row and skips ticks outside the track', () => {
    const p = tickPath([0, 100, 150, 99_999_999], { t0: 0, msPerPx: 100 }, 2, 14, 500);
    // 0, 1 and 1.5 px are closer than 2 px: one merged block.
    expect(p).toBe('M0 2h2.5v14h-2.5z');
    expect(tickPath([0, 1000], { t0: 0, msPerPx: 100 }, 0, 10, 500)).toBe('M0 0h1v10h-1zM10 0h1v10h-1z');
  });
});
