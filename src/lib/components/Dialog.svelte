<script lang="ts">
  // Modal dialog on the native <dialog> element: focus is trapped and Esc closes it.
  import type { Snippet } from 'svelte';
  import IconButton from './IconButton.svelte';

  let {
    open,
    title,
    width = 560,
    onclose,
    children,
    footer,
  }: { open: boolean; title: string; width?: number; onclose?: () => void; children?: Snippet; footer?: Snippet } = $props();

  let dlg: HTMLDialogElement | undefined = $state();

  $effect(() => {
    if (!dlg) return;
    if (open && !dlg.open) dlg.showModal();
    else if (!open && dlg.open) dlg.close();
  });
</script>

<dialog
  bind:this={dlg}
  style="--w: {width}px"
  aria-label={title}
  oncancel={(e) => {
    e.preventDefault();
    onclose?.();
  }}
  onclick={(e) => {
    if (e.target === dlg) onclose?.();
  }}
>
  {#if open}
    <div class="body">
      <header>
        <h2 class="t-title">{title}</h2>
        <IconButton icon="close" label="Close" onclick={() => onclose?.()} />
      </header>
      <div class="content">{@render children?.()}</div>
      {#if footer}<footer>{@render footer()}</footer>{/if}
    </div>
  {/if}
</dialog>

<style>
  dialog {
    width: min(var(--w), calc(100vw - 32px));
    max-height: calc(100vh - 64px);
    padding: 0;
    border: var(--border-width) solid var(--border-default);
    border-radius: var(--radius-lg);
    background: var(--surface-raised);
    color: var(--text-primary);
    box-shadow: var(--shadow-3);
    overflow: hidden;
  }
  dialog[open] { animation: as-slide-in var(--dur-2) var(--ease-out), as-fade-in var(--dur-fade) linear; }
  :global(:root[data-motion='reduced']) dialog[open] { animation: as-fade-in var(--dur-fade) linear; }
  dialog::backdrop { background: color-mix(in srgb, var(--shadow-color) 40%, transparent); }
  .body { display: flex; flex-direction: column; max-height: calc(100vh - 64px); }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    padding: var(--space-4) var(--space-4) var(--space-3) var(--space-6);
    border-bottom: var(--border-width) solid var(--border-subtle);
  }
  .content { padding: var(--space-4) var(--space-6) var(--space-6); overflow: auto; }
  footer { display: flex; justify-content: flex-end; gap: var(--space-2); padding: var(--space-3) var(--space-6); border-top: var(--border-width) solid var(--border-subtle); }
</style>
