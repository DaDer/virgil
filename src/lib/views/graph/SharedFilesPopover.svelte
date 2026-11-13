<script lang="ts">
  // Paths two agents share; each selects the file (ux-spec 3.2).
  import PathText from '../../components/PathText.svelte';

  let { files, x, y, cwd, onselect, onclose }: { files: string[]; x: number; y: number; cwd: string | null; onselect: (path: string) => void; onclose: () => void } = $props();

  let el: HTMLDivElement | undefined = $state();
  $effect(() => {
    const down = (e: PointerEvent) => {
      if (!el?.contains(e.target as Node)) onclose();
    };
    const key = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        onclose();
      }
    };
    window.addEventListener('pointerdown', down, true);
    window.addEventListener('keydown', key, true);
    queueMicrotask(() => el?.querySelector('button')?.focus());
    return () => {
      window.removeEventListener('pointerdown', down, true);
      window.removeEventListener('keydown', key, true);
    };
  });
  const left = $derived(Math.min(x + 8, (typeof window !== 'undefined' ? window.innerWidth : 1200) - 340));
  const top = $derived(Math.min(y + 8, (typeof window !== 'undefined' ? window.innerHeight : 800) - 260));
</script>

<div class="pop" bind:this={el} style="left: {left}px; top: {top}px" role="dialog" aria-label="Shared files">
  <p class="t-label head">{files.length} shared {files.length === 1 ? 'file' : 'files'}</p>
  <ul>
    {#each files as f (f)}
      <li><button type="button" class="row t-mono" onclick={() => onselect(f)}><PathText path={f} {cwd} max={40} /></button></li>
    {/each}
  </ul>
</div>

<style>
  .pop {
    position: fixed;
    z-index: var(--z-popover);
    width: 320px;
    max-height: 240px;
    overflow: auto;
    padding: var(--space-2);
    background: var(--surface-raised);
    border: var(--border-width) solid var(--border-default);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-3);
    animation: as-slide-in var(--dur-2) var(--ease-out);
  }
  .head { color: var(--text-muted); padding: 2px var(--space-2) var(--space-1); }
  .row { display: block; width: 100%; text-align: left; padding: 4px var(--space-2); border-radius: var(--radius-sm); color: var(--text-primary); }
  .row:hover { background: var(--surface-hover); }
</style>
