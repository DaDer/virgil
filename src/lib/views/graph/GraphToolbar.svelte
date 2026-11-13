<script lang="ts">
  // Floating toolbar, top left (ux-spec 3.2): Fit, zoom, Detail, Shared files, Edits only.
  import Button from '../../components/Button.svelte';
  import Menu from '../../components/Menu.svelte';
  import MenuItem from '../../components/MenuItem.svelte';
  import type { DetailSetting } from './buildGraph';

  let {
    zoom,
    detail,
    sharedFiles,
    sharedMode,
    linkNote,
    onfit,
    onzoomin,
    onzoomout,
    onzoomreset,
    ondetail,
    onshared,
    onsharedmode,
  }: {
    zoom: number;
    detail: DetailSetting;
    sharedFiles: boolean;
    sharedMode: 'edits' | 'all';
    linkNote?: string;
    onfit: () => void;
    onzoomin: () => void;
    onzoomout: () => void;
    onzoomreset: () => void;
    ondetail: (d: DetailSetting) => void;
    onshared: (on: boolean) => void;
    onsharedmode: (m: 'edits' | 'all') => void;
  } = $props();

  const DETAIL: { value: DetailSetting; label: string }[] = [
    { value: 'auto', label: 'Auto' },
    { value: 'full', label: 'Full' },
    { value: 'compact', label: 'Compact' },
    { value: 'dot', label: 'Dots' },
  ];

  let detailBtn: HTMLButtonElement | undefined = $state();
  let modeBtn: HTMLButtonElement | undefined = $state();
  let detailOpen = $state(false);
  let modeOpen = $state(false);
</script>

<div class="toolbar" role="toolbar" aria-label="Graph tools">
  <Button size="sm" icon="refresh" onclick={onfit} title="Fit (F)">Fit</Button>
  <span class="sep" aria-hidden="true"></span>
  <button type="button" class="step" aria-label="Zoom out" title="Zoom out (-)" onclick={onzoomout}>−</button>
  <button type="button" class="zoom num t-small" onclick={onzoomreset} title="Reset zoom (0)">{Math.round(zoom * 100)}%</button>
  <button type="button" class="step" aria-label="Zoom in" title="Zoom in (+)" onclick={onzoomin}>+</button>
  <span class="sep" aria-hidden="true"></span>
  <button type="button" class="menu-btn t-small" bind:this={detailBtn} aria-haspopup="menu" aria-expanded={detailOpen} onclick={() => (detailOpen = !detailOpen)}>
    Detail: {DETAIL.find((d) => d.value === detail)?.label}
  </button>
  <Menu anchor={detailBtn} open={detailOpen} onclose={() => (detailOpen = false)} label="Level of detail">
    {#each DETAIL as d (d.value)}
      <MenuItem checked={detail === d.value} onselect={() => ((detailOpen = false), ondetail(d.value))}>{d.label}</MenuItem>
    {/each}
  </Menu>
  <span class="sep" aria-hidden="true"></span>
  <label class="check t-small" title="Shared files (S)">
    <input type="checkbox" checked={sharedFiles} onchange={(e) => onshared(e.currentTarget.checked)} />
    Shared files
  </label>
  {#if sharedFiles}
    <button type="button" class="menu-btn t-small" bind:this={modeBtn} aria-haspopup="menu" aria-expanded={modeOpen} onclick={() => (modeOpen = !modeOpen)}>
      {sharedMode === 'edits' ? 'Edits only' : 'Reads and edits'}
    </button>
    <Menu anchor={modeBtn} open={modeOpen} onclose={() => (modeOpen = false)} label="Shared files filter">
      <MenuItem checked={sharedMode === 'edits'} onselect={() => ((modeOpen = false), onsharedmode('edits'))}>Edits only</MenuItem>
      <MenuItem checked={sharedMode === 'all'} onselect={() => ((modeOpen = false), onsharedmode('all'))}>Reads and edits</MenuItem>
    </Menu>
    {#if linkNote}<span class="note t-small">{linkNote}</span>{/if}
  {/if}
</div>

<style>
  .toolbar {
    position: absolute;
    top: var(--space-3);
    left: var(--space-3);
    z-index: var(--z-sticky);
    display: flex;
    align-items: center;
    gap: var(--space-1);
    padding: var(--space-1);
    background: var(--surface-raised);
    border: var(--border-width) solid var(--border-default);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-2);
    max-width: calc(100% - 2 * var(--space-3));
    flex-wrap: wrap;
  }
  .sep { width: 1px; height: 18px; background: var(--border-subtle); margin: 0 var(--space-1); }
  .zoom { min-width: 44px; height: var(--size-control); border-radius: var(--radius-sm); color: var(--text-secondary); }
  .step { width: var(--size-control); height: var(--size-control); border-radius: var(--radius-sm); font-size: var(--text-lg); color: var(--text-secondary); }
  .step:hover, .zoom:hover, .menu-btn:hover { background: var(--surface-hover); }
  .menu-btn { height: var(--size-control); padding: 0 var(--space-2); border-radius: var(--radius-sm); color: var(--text-primary); }
  .check { display: inline-flex; align-items: center; gap: 6px; padding: 0 var(--space-2); color: var(--text-primary); cursor: pointer; }
  .check input { accent-color: var(--accent); margin: 0; }
  .note { color: var(--text-muted); padding: 0 var(--space-2); }
</style>
