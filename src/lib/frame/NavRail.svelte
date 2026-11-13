<script lang="ts">
  // ux-spec 2.2: app mark, four destinations, Settings at the bottom.
  import Icon from '../components/Icon.svelte';
  import { ROUTE_LABELS, ROUTES, type Route } from '../stores/route.svelte';

  let {
    current,
    disabled = [],
    onnavigate,
    onsettings,
  }: { current: Route; disabled?: Route[]; onnavigate: (r: Route) => void; onsettings: () => void } = $props();
</script>

<nav class="rail" aria-label="Views">
  <div class="mark" title="Virgil"><Icon name="mark" size={28} label="Virgil" /></div>
  <ul>
    {#each ROUTES as r, i (r)}
      {@const off = disabled.includes(r)}
      <li>
        <button
          type="button"
          class="dest"
          class:current={current === r}
          aria-current={current === r ? 'page' : undefined}
          aria-disabled={off}
          title={off ? 'Pick a session first' : `${ROUTE_LABELS[r]} (${i + 1})`}
          onclick={() => !off && onnavigate(r)}
        >
          <Icon name={r} size={20} />
          <span class="lbl">{ROUTE_LABELS[r]}</span>
        </button>
      </li>
    {/each}
  </ul>
  <button type="button" class="dest settings" title="Settings (Mod+,)" onclick={onsettings}>
    <Icon name="settings" size={20} />
    <span class="lbl">Settings</span>
  </button>
</nav>

<style>
  .rail {
    grid-area: rail;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-3) 0 var(--space-3);
    background: var(--surface-panel);
    border-right: var(--border-width) solid var(--border-subtle);
    z-index: var(--z-panel);
  }
  .mark { color: var(--text-primary); margin-bottom: var(--space-3); }
  ul { display: flex; flex-direction: column; gap: var(--space-1); }
  .dest {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    width: 48px;
    height: 48px;
    border-radius: var(--radius-md);
    color: var(--text-secondary);
    transition: background-color var(--dur-1) var(--ease-out), color var(--dur-1) var(--ease-out);
  }
  .dest:hover:not([aria-disabled='true']) { background: var(--surface-hover); color: var(--text-primary); }
  .current { background: var(--accent-soft); color: var(--accent-text); }
  .current::before {
    content: '';
    position: absolute;
    left: -8px;
    top: 10px;
    bottom: 10px;
    width: 3px;
    border-radius: var(--radius-pill);
    background: var(--accent);
  }
  .dest[aria-disabled='true'] { opacity: 0.4; cursor: default; }
  .lbl { font-size: var(--text-xs); line-height: 1; font-weight: var(--weight-medium); }
  .settings { margin-top: auto; }
</style>
