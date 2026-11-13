<script lang="ts" module>
  export type OverviewSort = 'busy' | 'activity' | 'name';
</script>

<script lang="ts">
  import Icon from '../components/Icon.svelte';
  import Menu from '../components/Menu.svelte';
  import MenuItem from '../components/MenuItem.svelte';

  let {
    projects,
    project,
    sort,
    onproject,
    onsort,
  }: {
    projects: { name: string; count: number }[];
    project: string | null;
    sort: OverviewSort;
    onproject: (p: string | null) => void;
    onsort: (s: OverviewSort) => void;
  } = $props();

  const SORTS: { value: OverviewSort; label: string }[] = [
    { value: 'busy', label: 'Busy first' },
    { value: 'activity', label: 'Last activity' },
    { value: 'name', label: 'Name' },
  ];

  let projBtn: HTMLButtonElement | undefined = $state();
  let sortBtn: HTMLButtonElement | undefined = $state();
  let projOpen = $state(false);
  let sortOpen = $state(false);
</script>

<div class="toolbar" role="toolbar" aria-label="Overview filters">
  <button type="button" class="drop" bind:this={projBtn} aria-haspopup="menu" aria-expanded={projOpen} onclick={() => (projOpen = !projOpen)}>
    <Icon name="filter" size={14} />
    <span class="ellipsis">{project ?? 'All projects'}</span>
    <Icon name="chevron-down" size={14} />
  </button>
  <button type="button" class="drop" bind:this={sortBtn} aria-haspopup="menu" aria-expanded={sortOpen} onclick={() => (sortOpen = !sortOpen)}>
    <Icon name="sort" size={14} />
    <span>{SORTS.find((s) => s.value === sort)?.label}</span>
    <Icon name="chevron-down" size={14} />
  </button>
</div>

<Menu anchor={projBtn} open={projOpen} onclose={() => (projOpen = false)} label="Project" width={260}>
  <MenuItem checked={project == null} hint={String(projects.reduce((n, p) => n + p.count, 0))} onselect={() => { projOpen = false; onproject(null); }}>All projects</MenuItem>
  {#each projects as p (p.name)}
    <MenuItem checked={project === p.name} hint={String(p.count)} onselect={() => { projOpen = false; onproject(p.name); }}>
      <span class="t-mono ellipsis">{p.name}</span>
    </MenuItem>
  {/each}
</Menu>

<Menu anchor={sortBtn} open={sortOpen} onclose={() => (sortOpen = false)} label="Sort">
  {#each SORTS as s (s.value)}
    <MenuItem checked={sort === s.value} onselect={() => { sortOpen = false; onsort(s.value); }}>{s.label}</MenuItem>
  {/each}
</Menu>

<style>
  .toolbar { display: flex; align-items: center; gap: var(--space-2); }
  .drop {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    max-width: 260px;
    height: var(--size-control);
    padding: 0 var(--space-3);
    border-radius: var(--radius-sm);
    border: var(--border-width) solid var(--border-default);
    background: var(--surface-card);
    color: var(--text-primary);
    font-size: var(--text-sm);
    box-shadow: var(--shadow-1);
  }
  .drop:hover { background: var(--surface-hover); }
</style>
