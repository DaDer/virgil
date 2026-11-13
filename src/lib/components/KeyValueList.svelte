<script lang="ts" module>
  import type { Snippet } from 'svelte';

  export interface KeyValueItem {
    key: string;
    value: string | Snippet | null | undefined;
    mono?: boolean;
    copy?: boolean;
    title?: string;
  }
</script>

<script lang="ts">
  import { copyText } from './clipboard';

  let { items }: { items: KeyValueItem[] } = $props();
  let copied = $state<string | null>(null);

  async function copy(item: KeyValueItem) {
    if (typeof item.value !== 'string') return;
    if (await copyText(item.value)) {
      copied = item.key;
      setTimeout(() => (copied = null), 1200);
    }
  }
</script>

<dl class="kv">
  {#each items as item (item.key)}
    <dt class="t-small">{item.key}</dt>
    <dd class:t-mono={item.mono} title={item.title ?? (typeof item.value === 'string' ? item.value : undefined)}>
      {#if typeof item.value === 'function'}
        {@render item.value()}
      {:else if item.value == null || item.value === ''}
        <span class="muted">–</span>
      {:else if item.copy}
        <button type="button" class="copy" onclick={() => copy(item)} title="Copy">
          <span class="ellipsis">{item.value}</span>
          <span class="hint t-small">{copied === item.key ? 'Copied' : 'Copy'}</span>
        </button>
      {:else}
        <span class="val">{item.value}</span>
      {/if}
    </dd>
  {/each}
</dl>

<style>
  .kv { display: grid; grid-template-columns: minmax(84px, max-content) 1fr; gap: var(--space-1) var(--space-4); margin: 0; }
  dt { color: var(--text-muted); padding-top: 1px; }
  dd { margin: 0; min-width: 0; color: var(--text-primary); overflow-wrap: anywhere; }
  .copy { display: inline-flex; align-items: baseline; gap: var(--space-2); max-width: 100%; text-align: left; }
  .copy .hint { color: var(--text-muted); opacity: 0; transition: opacity var(--dur-1) var(--ease-out); }
  .copy:hover .hint, .copy:focus-visible .hint { opacity: 1; color: var(--accent-text); }
</style>
