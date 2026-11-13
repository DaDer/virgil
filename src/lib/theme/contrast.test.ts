// Parses the theme files and asserts the contrast rules of ux-spec 4.6 (WCAG 2.1).
import { describe, expect, test } from 'vitest';
import { readdirSync, readFileSync } from 'node:fs';
import { THEMES } from './theme';

const dir = new URL('./themes/', import.meta.url);
const files: Record<string, string> = Object.fromEntries(
  readdirSync(dir)
    .filter((f) => f.endsWith('.css'))
    .map((f) => [f, readFileSync(new URL(f, dir), 'utf8')]),
);

function blocks(css: string): Map<string, Record<string, string>> {
  const out = new Map<string, Record<string, string>>();
  const re = /:root\[data-theme="(\w+)"\]\[data-mode="(\w+)"\][^{]*\{([^}]*)\}/g;
  for (const m of css.matchAll(re)) {
    const vars: Record<string, string> = {};
    for (const d of m[3].matchAll(/--([\w-]+):\s*([^;]+);/g)) vars[d[1]] = d[2].trim();
    out.set(`${m[1]}/${m[2]}`, vars);
  }
  return out;
}

function lum(hex: string): number {
  const h = hex.replace('#', '');
  const c = [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16) / 255);
  const [r, g, b] = c.map((v) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}
function ratio(a: string, b: string): number {
  const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
}

const MAIN = ['surface-app', 'surface-canvas', 'surface-panel', 'surface-card'];
const STATE_SURFACES = [...MAIN, 'surface-raised'];
const EVERY = [...STATE_SURFACES, 'surface-sunken'];
const STATES = ['queued', 'running', 'done', 'error', 'stopped', 'idle'];
const MODELS = ['opus', 'sonnet', 'haiku', 'fable', 'other'];
const TYPES = ['session', 'workflow', 'general', 'fork', 'explore', 'plan', 'custom'];

const all = new Map<string, Record<string, string>>();
for (const css of Object.values(files)) for (const [k, v] of blocks(css)) all.set(k, v);

test('every theme has a light and a dark block', () => {
  for (const t of THEMES) {
    expect(all.has(`${t}/light`), `${t}/light`).toBe(true);
    expect(all.has(`${t}/dark`), `${t}/dark`).toBe(true);
  }
  expect(all.size).toBe(8);
});

describe.each([...all.entries()])('%s', (_name, v) => {
  const check = (fg: string, bgs: string[], min: number) => {
    const fails: string[] = [];
    for (const bg of bgs) {
      expect(v[fg], `--${fg} defined`).toMatch(/^#[0-9a-fA-F]{6}$/);
      expect(v[bg], `--${bg} defined`).toMatch(/^#[0-9a-fA-F]{6}$/);
      const r = ratio(v[fg], v[bg]);
      if (r < min) fails.push(`--${fg} on --${bg}: ${r.toFixed(2)} < ${min}`);
    }
    expect(fails).toEqual([]);
  };

  test('text', () => {
    check('text-primary', EVERY, 7);
    check('text-secondary', EVERY, 4.5);
    check('text-muted', EVERY, 4.5);
  });
  test('borders, accent and focus', () => {
    check('border-strong', MAIN, 3);
    check('accent-text', [...MAIN, 'accent-soft'], 4.5);
    check('focus-ring', MAIN, 3);
  });
  test('states', () => {
    for (const s of STATES) {
      check(`state-${s}`, STATE_SURFACES, 3);
      check(`state-${s}-text`, [...EVERY, `state-${s}-soft`], 4.5);
      check('text-primary', [`state-${s}-soft`], 7);
    }
  });
  test('models and types', () => {
    for (const m of MODELS) check(`model-${m}`, MAIN, 3);
    for (const t of TYPES) check(`type-${t}`, MAIN, 3);
  });
});
