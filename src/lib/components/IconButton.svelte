<script lang="ts">
  import type { HTMLButtonAttributes } from 'svelte/elements';
  import type { IconName } from './icons';
  import Icon from './Icon.svelte';

  let {
    icon,
    label,
    pressed,
    size = 'md',
    disabled = false,
    ...rest
  }: { icon: IconName; label: string; pressed?: boolean; size?: 'sm' | 'md'; disabled?: boolean } & Omit<HTMLButtonAttributes, 'disabled'> = $props();
</script>

<button
  type="button"
  class="ib {size}"
  class:on={pressed}
  aria-label={label}
  aria-pressed={pressed}
  title={label}
  {disabled}
  {...rest}
>
  <Icon name={icon} size={size === 'sm' ? 14 : 16} />
</button>

<style>
  .ib {
    display: inline-grid;
    place-items: center;
    width: var(--size-control);
    height: var(--size-control);
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    transition: background-color var(--dur-1) var(--ease-out), color var(--dur-1) var(--ease-out);
  }
  .sm { width: 22px; height: 22px; }
  .ib:hover:not(:disabled) { background: var(--surface-hover); color: var(--text-primary); }
  .on { background: var(--accent-soft); color: var(--accent-text); }
  .ib:disabled { opacity: 0.4; }
</style>
