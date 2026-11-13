// Shared stores (ux-spec 1.4 and 6.1). Views import from '$lib/stores'.
export { settings } from './settings.svelte';
export { selection, hover, sel, sameSelection, type Selection, type SelectionKind } from './selection.svelte';
export { route, scope, ROUTES, ROUTE_LABELS, type Route, type Scope } from './route.svelte';
export { clock, SPEEDS } from './clock.svelte';
export { search, agentKey } from './search.svelte';
export { world, type Problem, type WorldPhase } from './world.svelte';
export { ui } from './ui.svelte';
export { actions } from './actions';
