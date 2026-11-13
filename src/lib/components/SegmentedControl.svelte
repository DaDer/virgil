<script lang="ts" generics="T extends string">
  import type { Snippet } from 'svelte';

  let {
    options,
    value,
    onchange,
    label,
    size = 'md',
    item,
  }: {
    options: { value: T; label: string; disabled?: boolean }[];
    value: T;
    onchange?: (v: T) => void;
    label: string;
    size?: 'sm' | 'md';
    item?: Snippet<[{ value: T; label: string; selected: boolean }]>;
  } = $props();

  let root: HTMLDivElement;

  function key(e: KeyboardEvent) {
    if (e.key !== 'ArrowLeft' && e.key !== 'ArrowRight') return;
    e.preventDefault();
    const enabled = options.filter((o) => !o.disabled);
    const i = enabled.findIndex((o) => o.value === value);
    const next = enabled[(i + (e.key === 'ArrowRight' ? 1 : -1) + enabled.length) % enabled.length];
    if (next) {
      onchange?.(next.value);
      queueMicrotask(() => root.querySelector<HTMLButtonElement>('[aria-checked="true"]')?.focus());
    }
  }
</script>

<div class="seg {size}" role="radiogroup" aria-label={label} bind:this={root} onkeydown={key} tabindex="-1">
  {#each options as o (o.value)}
    <button
      type="button"
      role="radio"
      aria-checked={o.value === value}
      tabindex={o.value === value ? 0 : -1}
      disabled={o.disabled}
      onclick={() => onchange?.(o.value)}
    >
      {#if item}{@render item({ ...o, selected: o.value === value })}{:else}{o.label}{/if}
    </button>
  {/each}
</div>

<style>
  .seg {
    display: inline-flex;
    padding: 2px;
    gap: 2px;
    border-radius: var(--radius-md);
    background: var(--surface-sunken);
    border: var(--border-width) solid var(--border-subtle);
  }
  button {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    height: 24px;
    padding: 0 var(--pad-3);
    border-radius: var(--radius-sm);
    font-size: var(--text-sm);
    font-weight: var(--weight-medium);
    color: var(--text-secondary);
    white-space: nowrap;
    transition: background-color var(--dur-1) var(--ease-out), color var(--dur-1) var(--ease-out);
  }
  .sm button { height: 20px; padding: 0 var(--pad-2); font-size: var(--text-xs); }
  button:hover:not(:disabled) { color: var(--text-primary); }
  button[aria-checked='true'] { background: var(--surface-card); color: var(--text-primary); box-shadow: var(--shadow-1), inset 0 0 0 var(--border-width) var(--border-default); }
  button:disabled { opacity: 0.4; }
</style>
