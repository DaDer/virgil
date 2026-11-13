<script lang="ts">
  // A floating panel anchored to an element. Closes on outside click and Esc.
  import type { Snippet } from 'svelte';

  type Placement = 'bottom-start' | 'bottom-end' | 'top-start' | 'top-end' | 'right-start';

  let {
    anchor,
    open,
    placement = 'bottom-start',
    onclose,
    label,
    role = 'dialog',
    width,
    children,
  }: {
    anchor: HTMLElement | null | undefined;
    open: boolean;
    placement?: Placement;
    onclose?: () => void;
    label?: string;
    role?: string;
    width?: number;
    children?: Snippet;
  } = $props();

  let el: HTMLDivElement | undefined = $state();
  let pos = $state({ left: 0, top: 0 });

  function place() {
    if (!anchor || !el) return;
    const a = anchor.getBoundingClientRect();
    const r = el.getBoundingClientRect();
    const gap = 6;
    let left = placement.endsWith('end') ? a.right - r.width : a.left;
    let top = placement.startsWith('top') ? a.top - r.height - gap : a.bottom + gap;
    if (placement === 'right-start') {
      left = a.right + gap;
      top = a.top;
    }
    left = Math.max(8, Math.min(left, window.innerWidth - r.width - 8));
    top = Math.max(8, Math.min(top, window.innerHeight - r.height - 8));
    pos = { left, top };
  }

  $effect(() => {
    if (!open) return;
    queueMicrotask(place);
    const down = (e: PointerEvent) => {
      const t = e.target as Node;
      if (el?.contains(t) || anchor?.contains(t)) return;
      onclose?.();
    };
    const key = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        e.preventDefault();
        onclose?.();
        anchor?.focus();
      }
    };
    const re = () => place();
    window.addEventListener('pointerdown', down, true);
    window.addEventListener('keydown', key, true);
    window.addEventListener('resize', re);
    return () => {
      window.removeEventListener('pointerdown', down, true);
      window.removeEventListener('keydown', key, true);
      window.removeEventListener('resize', re);
    };
  });
</script>

{#if open}
  <div
    class="pop"
    bind:this={el}
    {role}
    aria-label={label}
    style="left: {pos.left}px; top: {pos.top}px; {width ? `width: ${width}px` : ''}"
  >
    {@render children?.()}
  </div>
{/if}

<style>
  .pop {
    position: fixed;
    z-index: var(--z-popover);
    min-width: 180px;
    max-height: min(480px, calc(100vh - 16px));
    overflow: auto;
    padding: var(--space-1);
    background: var(--surface-raised);
    color: var(--text-primary);
    border: var(--border-width) solid var(--border-default);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-3);
    animation: as-slide-in var(--dur-2) var(--ease-out), as-fade-in var(--dur-fade) linear;
  }
  :global(:root[data-motion='reduced']) .pop { animation: as-fade-in var(--dur-fade) linear; }
</style>
