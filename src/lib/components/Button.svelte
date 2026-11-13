<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { HTMLButtonAttributes } from 'svelte/elements';
  import type { IconName } from './icons';
  import Icon from './Icon.svelte';

  let {
    variant = 'secondary',
    size = 'md',
    icon,
    disabled = false,
    children,
    ...rest
  }: {
    variant?: 'primary' | 'secondary' | 'ghost';
    size?: 'sm' | 'md';
    icon?: IconName;
    disabled?: boolean;
    children?: Snippet;
  } & Omit<HTMLButtonAttributes, 'disabled'> = $props();
</script>

<button class="btn {variant} {size}" {disabled} type="button" {...rest}>
  {#if icon}<Icon name={icon} size={size === 'sm' ? 14 : 16} />{/if}
  {@render children?.()}
</button>

<style>
  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-2);
    height: var(--size-control);
    padding: 0 var(--pad-3);
    border-radius: var(--radius-sm);
    font-size: var(--text-md);
    font-weight: var(--weight-medium);
    white-space: nowrap;
    border: var(--border-width) solid transparent;
    transition: background-color var(--dur-1) var(--ease-out), border-color var(--dur-1) var(--ease-out), box-shadow var(--dur-1) var(--ease-out), transform var(--dur-1) var(--ease-out);
  }
  .sm { height: 24px; padding: 0 var(--pad-2); font-size: var(--text-sm); }
  .primary { background: var(--accent); color: var(--text-on-accent); border-color: var(--accent); }
  .primary:hover:not(:disabled) { background: var(--accent-hover); }
  .secondary { background: var(--surface-card); color: var(--text-primary); border-color: var(--border-default); box-shadow: var(--shadow-1); }
  .secondary:hover:not(:disabled) { background: var(--surface-hover); }
  .ghost { color: var(--text-secondary); }
  .ghost:hover:not(:disabled) { background: var(--surface-hover); color: var(--text-primary); }
  .btn:disabled { opacity: 0.45; }
  :global(:root[data-theme="poster"]) .btn:not(.ghost):hover:not(:disabled) { transform: translate(-1px, -1px); box-shadow: var(--shadow-2); }
  :global(:root[data-theme="poster"]) .btn:not(.ghost):active:not(:disabled) { transform: translate(2px, 2px); box-shadow: var(--shadow-0); }
</style>
