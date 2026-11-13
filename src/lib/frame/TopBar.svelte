<script lang="ts">
  // ux-spec 2.3: scope crumbs, search, Live/Replay switch, Inspector toggle.
  import IconButton from '../components/IconButton.svelte';
  import { actions } from '../stores/actions';
  import { route, scope } from '../stores/route.svelte';
  import { search } from '../stores/search.svelte';
  import { ui } from '../stores/ui.svelte';
  import { world } from '../stores/world.svelte';
  import ModeSwitch from './ModeSwitch.svelte';
  import ScopeCrumbs from './ScopeCrumbs.svelte';
  import SearchField from './SearchField.svelte';

  let input: HTMLInputElement | undefined = $state();
  $effect(() => {
    search.input = input ?? null;
  });

  const runs = $derived(Object.values(world.session(scope.sessionId)?.runs ?? {}));
</script>

<header class="top">
  <ScopeCrumbs
    sessions={world.sessions}
    {runs}
    scope={scope.value}
    showSession={route.current !== 'overview'}
    onoverview={() => route.navigate('overview')}
    onsession={(id) => {
      scope.openSession(id);
      if (route.current === 'overview') route.navigate('graph');
    }}
    onrun={(id) => scope.openRun(id)}
  />
  <div class="space"></div>
  <SearchField bind:value={search.text} bind:input count={search.count} />
  <ModeSwitch
    mode={world.mode}
    recordingName={world.status?.recording?.name}
    onlive={() => actions.goLive()}
    onreplay={() => actions.chooseReplay()}
  />
  <IconButton icon="panel" label="Inspector (Mod+I)" pressed={ui.inspectorOpen} onclick={() => (ui.inspectorOpen = !ui.inspectorOpen)} />
</header>

<style>
  .top {
    grid-area: top;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    height: var(--size-topbar);
    padding: 0 var(--space-3) 0 var(--space-4);
    background: var(--surface-panel);
    border-bottom: var(--border-width) solid var(--border-subtle);
    min-width: 0;
    z-index: var(--z-panel);
  }
  .space { flex: 1; }
</style>
