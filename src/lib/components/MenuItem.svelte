<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { IconName } from './icons';
  import Icon from './Icon.svelte';

  let {
    onselect,
    icon,
    checked,
    disabled = false,
    hint,
    children,
    leading,
  }: {
    onselect?: () => void;
    icon?: IconName;
    /** Shows a check mark and makes this a menuitemradio. */
    checked?: boolean;
    disabled?: boolean;
    hint?: string;
    children?: Snippet;
    leading?: Snippet;
  } = $props();
</script>

<button
  type="button"
  class="item"
  role={checked === undefined ? 'menuitem' : 'menuitemradio'}
  aria-checked={checked}
  tabindex="-1"
  {disabled}
  onclick={() => onselect?.()}
>
  {#if checked !== undefined}
    <span class="check">{#if checked}<Icon name="check" size={14} />{/if}</span>
  {/if}
  {#if leading}{@render leading()}{:else if icon}<Icon name={icon} size={16} />{/if}
  <span class="label">{@render children?.()}</span>
  {#if hint}<span class="hint t-small">{hint}</span>{/if}
</button>

<style>
  .item {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    width: 100%;
    min-height: 30px;
    padding: var(--space-1) var(--space-2);
    border-radius: var(--radius-sm);
    text-align: left;
    color: var(--text-primary);
  }
  .item:hover:not(:disabled), .item:focus-visible { background: var(--surface-hover); outline-offset: -2px; }
  .item:disabled { opacity: 0.45; }
  .label { flex: 1; min-width: 0; display: flex; align-items: center; gap: var(--space-2); }
  .hint { color: var(--text-muted); }
  .check { width: 14px; flex: none; color: var(--accent-text); }
</style>
