// View-wide state that node and edge components read directly, so the Svelte Flow node and
// edge arrays change only on layout (ux-spec 5.3: keep node components cheap, batch updates).

import type { FileTouch } from '../../contract';

export interface LinkPopover {
  x: number;
  y: number;
  files: string[];
}

class GraphState {
  sessionId = $state<string | null>(null);
  zoom = $state(1);
  /** Running agents allowed to loop (motion budget). */
  loops = $state.raw<Set<string>>(new Set());
  /** Edge ids on the selected agent's parent chain. */
  chain = $state.raw<Set<string>>(new Set());
  /** Hover: the hovered agent and its neighbours; others dim to 55%. Null when nothing hovered. */
  related = $state.raw<Set<string> | null>(null);
  hoverEdges = $state.raw<Set<string>>(new Set());
  /** A selected file: who touched it, with R or E. Others dim to 40%. */
  fileTouch = $state.raw<Map<string, FileTouch> | null>(null);
  linkPopover = $state.raw<LinkPopover | null>(null);
  /** Hovered agent for the tooltip, with the pointer position. */
  tip = $state.raw<{ agentId: string; x: number; y: number } | null>(null);
  collapsedToggle: (runId: string) => void = () => {};
}

export const graph = new GraphState();
