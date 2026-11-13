<script lang="ts">
  // A path relative to cwd (or with ~), middle-truncated, full path in the tooltip.
  import { displayPath, middleTruncate } from '../format';

  let { path, cwd, max = 48 }: { path: string; cwd?: string | null; max?: number } = $props();
  const shown = $derived(middleTruncate(displayPath(path, cwd), max));
</script>

<span class="path t-mono" title={path}>{shown}</span>

<style>
  .path { white-space: nowrap; min-width: 0; overflow: hidden; text-overflow: ellipsis; }
</style>
