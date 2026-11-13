// Inspector section collapse state, remembered per section name (ux-spec 3.5). A per-viewer
// convenience in localStorage; everything works without it.

const KEY = 'virgil.inspector.sections.v1';

function load(): Record<string, boolean> {
  try {
    const raw = globalThis.localStorage?.getItem(KEY);
    const v = raw ? JSON.parse(raw) : {};
    return v && typeof v === 'object' ? v : {};
  } catch {
    return {};
  }
}

class Sections {
  collapsed = $state<Record<string, boolean>>(load());

  toggle(name: string, defaultCollapsed = false): void {
    this.collapsed[name] = !(this.collapsed[name] ?? defaultCollapsed);
    try {
      globalThis.localStorage?.setItem(KEY, JSON.stringify(this.collapsed));
    } catch {
      /* storage unavailable */
    }
  }

  isCollapsed(name: string, defaultCollapsed = false): boolean {
    return this.collapsed[name] ?? defaultCollapsed;
  }
}

export const sections = new Sections();
