// Global keyboard shortcuts (ux-spec 2.9). View-level keys (arrows inside a view, Enter, F, + - 0,
// S) belong to the views: they handle them on their own root and call preventDefault, and this
// handler skips events whose default was prevented.

import { actions } from '../stores/actions';
import { clock } from '../stores/clock.svelte';
import { ROUTES, route } from '../stores/route.svelte';
import { search } from '../stores/search.svelte';
import { ui } from '../stores/ui.svelte';
import { world } from '../stores/world.svelte';

export const IS_MAC = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);
export const MOD_LABEL = IS_MAC ? '⌘' : 'Ctrl';

export const SHORTCUTS: { keys: string[]; action: string }[] = [
  { keys: ['1', '2', '3', '4'], action: 'Overview, Graph, Workflow, Timeline' },
  { keys: ['Mod K', '/'], action: 'Focus search' },
  { keys: ['Mod I'], action: 'Toggle the Inspector' },
  { keys: ['Mod ,'], action: 'Settings' },
  { keys: ['Esc'], action: 'Close dialog or menu, else clear search, else clear selection' },
  { keys: ['L', 'R'], action: 'Switch to Live / Replay' },
  { keys: ['Space'], action: 'Play or pause (replay)' },
  { keys: [',', '.'], action: 'Previous / next event (replay)' },
  { keys: ['[', ']'], action: 'Slower / faster (replay)' },
  { keys: ['Home', 'End'], action: 'Start / end of the recording (replay)' },
  { keys: ['←', '→'], action: 'Seek 5 s (replay), with Shift 1 min' },
  { keys: ['Arrows'], action: 'Move the selection inside a view' },
  { keys: ['Enter'], action: 'Open the selected session; focus the Inspector' },
  { keys: ['F'], action: 'Fit everything in view (Graph, Timeline)' },
  { keys: ['+', '-', '0'], action: 'Zoom in, out, reset (Graph, Timeline)' },
  { keys: ['S'], action: 'Toggle the shared-files overlay (Graph)' },
  { keys: ['P'], action: 'Select the parent of the selected agent' },
  { keys: ['?'], action: 'Show this sheet' },
];

function typing(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  if (!el) return false;
  const tag = el.tagName;
  return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || el.isContentEditable;
}

/** Keydown handler for the window. Returns true when it handled the key. */
export function handleGlobalKey(e: KeyboardEvent): boolean {
  const mod = IS_MAC ? e.metaKey : e.ctrlKey;
  const key = e.key;
  const done = () => {
    e.preventDefault();
    return true;
  };

  if (key === 'Escape') {
    if (e.defaultPrevented) return false;
    actions.escape();
    if (typing(e.target)) (e.target as HTMLElement).blur();
    return done();
  }
  if (mod && !e.altKey) {
    const k = key.toLowerCase();
    if (k === 'k') return search.focus(), done();
    if (k === 'i') return (ui.inspectorOpen = !ui.inspectorOpen), done();
    if (k === ',') return (ui.settingsOpen = true), done();
    return false;
  }
  if (e.defaultPrevented || typing(e.target) || ui.anyDialog || e.altKey || e.ctrlKey || e.metaKey) return false;

  const idx = ['1', '2', '3', '4'].indexOf(key);
  if (idx >= 0) return route.navigate(ROUTES[idx]), done();
  if (key === '/') return search.focus(), done();
  if (key === '?') return (ui.shortcutsOpen = true), done();
  if (key === 'l' || key === 'L') return void actions.goLive(), done();
  if (key === 'r' || key === 'R') return actions.chooseReplay(), done();
  if (key === 'p' || key === 'P') return actions.selectParent(), done();
  if (key === 'Enter') {
    const insp = document.querySelector<HTMLElement>('[data-inspector]');
    if (insp && route.current !== 'overview') return insp.focus(), done();
    return false;
  }

  if (world.mode !== 'replay') return false;
  if (key === ' ') {
    // A focused button handles Space itself.
    if ((e.target as HTMLElement | null)?.closest?.('button, [role="button"], [role="slider"]')) return false;
    return void actions.togglePlay(), done();
  }
  if (key === ',') return void clock.step(-1), done();
  if (key === '.') return void clock.step(1), done();
  if (key === '[') return void clock.stepSpeed(-1), done();
  if (key === ']') return void clock.stepSpeed(1), done();
  if (key === 'Home') return void clock.toStart(), done();
  if (key === 'End') return void clock.toEnd(), done();
  if (key === 'ArrowLeft') return void clock.nudge(e.shiftKey ? -60_000 : -5_000), done();
  if (key === 'ArrowRight') return void clock.nudge(e.shiftKey ? 60_000 : 5_000), done();
  return false;
}
