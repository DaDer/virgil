<script lang="ts">
  // Files touched: edits first then reads, at most `limit` with Show all (ux-spec 3.5).
  import PathText from '../../components/PathText.svelte';
  import type { FileItem } from './model';

  let { files, cwd, limit = 50, onselect }: { files: FileItem[]; cwd: string | null; limit?: number; onselect: (path: string) => void } = $props();
  let all = $state(false);
  const shown = $derived(all ? files : files.slice(0, limit));
</script>

<ul class="files">
  {#each shown as f (f.path)}
    <li>
      <button type="button" class="row" onclick={() => onselect(f.path)} title={f.viaBash ? `${f.path} (seen in a shell command)` : f.path}>
        <span class="badge" class:edit={f.mode === 'E'}>{f.mode}</span>
        <span class="path t-mono"><PathText path={f.path} {cwd} max={38} /></span>
        {#if f.count > 1}<span class="n num">×{f.count}</span>{/if}
        {#if f.others > 0}<span class="others num" title="{f.others} other agents touched it">+{f.others}</span>{/if}
      </button>
    </li>
  {/each}
</ul>
{#if files.length > limit}
  <button type="button" class="more t-small" onclick={() => (all = !all)}>{all ? 'Show less' : `Show all ${files.length}`}</button>
{/if}

<style>
  .files { display: flex; flex-direction: column; }
  .row { display: flex; align-items: center; gap: var(--space-2); width: 100%; min-height: 26px; padding: 0 var(--space-1); border-radius: var(--radius-sm); text-align: left; color: var(--text-primary); }
  .row:hover { background: var(--surface-hover); }
  .badge { flex: none; display: inline-grid; place-items: center; width: 16px; height: 16px; border-radius: var(--radius-xs); background: var(--surface-sunken); color: var(--text-secondary); font-size: var(--text-xs); font-weight: var(--weight-semibold); }
  .badge.edit { background: var(--accent-soft); color: var(--accent-text); }
  .path { flex: 1; min-width: 0; overflow: hidden; white-space: nowrap; font-size: var(--text-xs); }
  .n { color: var(--text-muted); font-size: var(--text-xs); }
  .others { padding: 0 5px; border-radius: var(--radius-pill); background: var(--surface-sunken); color: var(--text-secondary); font-size: var(--text-xs); }
  .more { align-self: flex-start; color: var(--accent-text); }
</style>
