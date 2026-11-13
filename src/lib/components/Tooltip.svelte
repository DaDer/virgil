<script lang="ts">
  // Hover and focus tooltip after `delay` ms. Text or a `tip` snippet.
  import type { Snippet } from 'svelte';

  let {
    text,
    delay = 400,
    tip,
    children,
    block = false,
  }: { text?: string; delay?: number; tip?: Snippet; children?: Snippet; block?: boolean } = $props();

  let host: HTMLSpanElement | undefined = $state();
  let shown = $state(false);
  let pos = $state({ x: 0, y: 0, below: false });
  let timer: ReturnType<typeof setTimeout> | null = null;
  const id = `tip-${Math.random().toString(36).slice(2, 9)}`;

  function show() {
    if (!text && !tip) return;
    timer = setTimeout(() => {
      if (!host) return;
      const r = host.getBoundingClientRect();
      const below = r.top < 60;
      pos = { x: r.left + r.width / 2, y: below ? r.bottom + 6 : r.top - 6, below };
      shown = true;
    }, delay);
  }
  function hide() {
    if (timer) clearTimeout(timer);
    timer = null;
    shown = false;
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<span
  class="host"
  class:block
  bind:this={host}
  onpointerenter={show}
  onpointerleave={hide}
  onfocusin={show}
  onfocusout={hide}
  aria-describedby={shown ? id : undefined}
>
  {@render children?.()}
</span>
{#if shown}
  <span class="tip" class:below={pos.below} role="tooltip" {id} style="left: {pos.x}px; top: {pos.y}px">
    {#if tip}{@render tip()}{:else}{text}{/if}
  </span>
{/if}

<style>
  .host { display: inline-flex; min-width: 0; }
  .block { display: flex; }
  .tip {
    position: fixed;
    z-index: var(--z-toast);
    transform: translate(-50%, -100%);
    max-width: 320px;
    padding: var(--space-1) var(--space-2);
    border-radius: var(--radius-sm);
    background: var(--surface-raised);
    color: var(--text-primary);
    border: var(--border-width) solid var(--border-default);
    box-shadow: var(--shadow-3);
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
    pointer-events: none;
    white-space: pre-line;
    overflow-wrap: anywhere;
    animation: as-fade-in var(--dur-fade) linear;
  }
  .tip.below { transform: translate(-50%, 0); }
</style>
