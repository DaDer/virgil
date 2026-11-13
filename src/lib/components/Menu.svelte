<script lang="ts">
  // A menu in a popover, with arrow-key movement between MenuItems.
  import type { Snippet } from 'svelte';
  import Popover from './Popover.svelte';

  let {
    anchor,
    open,
    onclose,
    label,
    placement = 'bottom-start',
    width,
    children,
  }: {
    anchor: HTMLElement | null | undefined;
    open: boolean;
    onclose?: () => void;
    label: string;
    placement?: 'bottom-start' | 'bottom-end' | 'top-start' | 'top-end';
    width?: number;
    children?: Snippet;
  } = $props();

  let list: HTMLDivElement | undefined = $state();

  $effect(() => {
    if (open && list) queueMicrotask(() => list?.querySelector<HTMLElement>('[role^="menuitem"]:not([disabled])')?.focus());
  });

  function key(e: KeyboardEvent) {
    const items = [...(list?.querySelectorAll<HTMLElement>('[role^="menuitem"]:not([disabled])') ?? [])];
    const i = items.indexOf(document.activeElement as HTMLElement);
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      const n = items.length;
      items[(i + (e.key === 'ArrowDown' ? 1 : -1) + n) % n]?.focus();
    } else if (e.key === 'Home') {
      e.preventDefault();
      items[0]?.focus();
    } else if (e.key === 'End') {
      e.preventDefault();
      items.at(-1)?.focus();
    } else if (e.key === 'Tab') {
      onclose?.();
    }
  }
</script>

<Popover {anchor} {open} {onclose} {placement} {label} {width} role="presentation">
  <!-- svelte-ignore a11y_interactive_supports_focus -->
  <div class="menu" role="menu" aria-label={label} bind:this={list} onkeydown={key}>
    {@render children?.()}
  </div>
</Popover>

<style>
  .menu { display: flex; flex-direction: column; gap: 1px; }
</style>
