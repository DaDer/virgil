<script lang="ts">
  // ux-spec 2.3: 240 px search with a "7 of 58" count; Mod+K or / focuses it; Esc clears.
  import Icon from '../components/Icon.svelte';

  let {
    value = $bindable(''),
    count,
    placeholder = 'Search agents, tasks, files',
    input = $bindable(),
  }: { value?: string; count?: { shown: number; total: number }; placeholder?: string; input?: HTMLInputElement } = $props();
</script>

<label class="search">
  <Icon name="search" size={14} />
  <span class="sr-only">Search</span>
  <input
    bind:this={input}
    bind:value
    type="search"
    {placeholder}
    spellcheck="false"
    autocomplete="off"
    onkeydown={(e) => {
      if (e.key === 'Escape' && value) {
        e.stopPropagation();
        value = '';
      }
    }}
  />
  {#if count}<span class="count t-small num" aria-live="polite">{count.shown} of {count.total}</span>{:else}<kbd class="t-small">/</kbd>{/if}
</label>

<style>
  .search {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    width: 240px;
    height: var(--size-control);
    padding: 0 var(--space-2);
    border-radius: var(--radius-sm);
    background: var(--surface-sunken);
    border: var(--border-width) solid var(--border-subtle);
    color: var(--text-muted);
  }
  .search:focus-within { border-color: var(--focus-ring); }
  input { flex: 1; min-width: 0; border: 0; background: none; outline: none; color: var(--text-primary); font-size: var(--text-sm); }
  input::placeholder { color: var(--text-muted); }
  input::-webkit-search-cancel-button { display: none; }
  .count { color: var(--text-secondary); white-space: nowrap; }
  kbd { font-family: var(--font-mono); padding: 0 5px; border-radius: var(--radius-xs); border: var(--border-width) solid var(--border-default); color: var(--text-muted); }
</style>
