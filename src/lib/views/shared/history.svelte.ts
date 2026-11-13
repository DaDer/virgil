// Read side of the tool history the world store keeps (see toolHistory.ts). `version` changes
// whenever calls arrive, so views that read `ticks` re-run.

import { world } from '../../stores/world.svelte';
import type { ToolTick } from './toolHistory';

class HistoryStore {
  get version(): number {
    return world.toolsVersion;
  }

  ticks(sessionId: string, agentId: string): ToolTick[] {
    void world.toolsVersion;
    return world.tools.ticks(sessionId, agentId);
  }
}

export const history = new HistoryStore();
