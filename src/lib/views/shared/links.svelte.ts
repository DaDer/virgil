// Requests from one view to another: "open the Graph centred on this agent",
// "open the Timeline scrolled to this row". The target view consumes the request.

export interface FocusRequest {
  sessionId: string;
  agentId: string;
  nonce: number;
}

class ViewLinks {
  graphFocus = $state.raw<FocusRequest | null>(null);
  timelineFocus = $state.raw<FocusRequest | null>(null);
  private n = 0;

  focusInGraph(sessionId: string, agentId: string): void {
    this.graphFocus = { sessionId, agentId, nonce: ++this.n };
  }

  focusInTimeline(sessionId: string, agentId: string): void {
    this.timelineFocus = { sessionId, agentId, nonce: ++this.n };
  }
}

export const links = new ViewLinks();
