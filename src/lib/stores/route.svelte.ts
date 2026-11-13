// Current destination (ux-spec 2.2) and which session/run the views look at (1.4).

export type Route = 'overview' | 'graph' | 'workflow' | 'timeline';
export const ROUTES: readonly Route[] = ['overview', 'graph', 'workflow', 'timeline'];
export const ROUTE_LABELS: Record<Route, string> = {
  overview: 'Overview',
  graph: 'Graph',
  workflow: 'Workflow',
  timeline: 'Timeline',
};

class RouteStore {
  current = $state<Route>('overview');

  navigate(r: Route): void {
    if (r !== 'overview' && !scope.sessionId) return;
    this.current = r;
  }
}

export interface Scope {
  sessionId: string | null;
  runId: string | null;
}

class ScopeStore {
  sessionId = $state<string | null>(null);
  runId = $state<string | null>(null);

  get value(): Scope {
    return { sessionId: this.sessionId, runId: this.runId };
  }

  /** Scope to a session. `runId` defaults to null; views pick the current run when it is null. */
  openSession(id: string | null, runId: string | null = null): void {
    this.sessionId = id;
    this.runId = runId;
  }

  openRun(id: string | null): void {
    this.runId = id;
  }

  set(s: Scope): void {
    this.sessionId = s.sessionId;
    this.runId = s.runId;
  }
}

export const route = new RouteStore();
export const scope = new ScopeStore();
