<script lang="ts">
  import Dialog from '../components/Dialog.svelte';
  import { MOD_LABEL, SHORTCUTS } from './shortcuts';

  let { open, onclose }: { open: boolean; onclose: () => void } = $props();
</script>

<Dialog {open} title="Keyboard shortcuts" width={560} {onclose}>
  <dl class="sheet">
    {#each SHORTCUTS as s (s.action)}
      <dt>
        {#each s.keys as k, i (i)}
          {#if i > 0}<span class="or">or</span>{/if}
          {#each k.replace('Mod', MOD_LABEL).split(' ') as part, j (j)}<kbd>{part}</kbd>{/each}
        {/each}
      </dt>
      <dd>{s.action}</dd>
    {/each}
  </dl>
</Dialog>

<style>
  .sheet { display: grid; grid-template-columns: max-content 1fr; gap: var(--space-2) var(--space-6); margin: 0; }
  dt { display: flex; align-items: center; gap: 4px; flex-wrap: wrap; }
  dd { margin: 0; color: var(--text-secondary); }
  .or { color: var(--text-muted); font-size: var(--text-xs); }
  kbd {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    min-width: 22px;
    text-align: center;
    padding: 1px 6px;
    border-radius: var(--radius-xs);
    border: var(--border-width) solid var(--border-default);
    border-bottom-width: 2px;
    background: var(--surface-card);
    color: var(--text-primary);
  }
</style>
