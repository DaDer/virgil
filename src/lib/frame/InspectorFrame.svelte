<script lang="ts">
  // ux-spec 2.5: right panel with a header, a resize handle on its left edge, and a close button.
  // Below 1100 px window width it overlays the view (the frame sets `overlay`).
  import type { Snippet } from 'svelte';
  import IconButton from '../components/IconButton.svelte';
  import { INSPECTOR_MAX, INSPECTOR_MIN } from '../theme/theme';

  let {
    open,
    width,
    title,
    overlay = false,
    onclose,
    onresize,
    header,
    children,
  }: {
    open: boolean;
    width: number;
    title: string;
    overlay?: boolean;
    onclose: () => void;
    onresize: (w: number) => void;
    /** Glyph and state badge, drawn left and right of the title. */
    header?: Snippet<[]>;
    children?: Snippet;
  } = $props();

  let panel: HTMLElement | undefined = $state();
  let dragging = $state(false);

  function down(e: PointerEvent) {
    e.preventDefault();
    dragging = true;
    const startX = e.clientX;
    const startW = width;
    const move = (ev: PointerEvent) => onresize(Math.min(INSPECTOR_MAX, Math.max(INSPECTOR_MIN, startW + (startX - ev.clientX))));
    const up = () => {
      dragging = false;
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
  }

  function key(e: KeyboardEvent) {
    if (e.key === 'ArrowLeft') onresize(Math.min(INSPECTOR_MAX, width + 16));
    else if (e.key === 'ArrowRight') onresize(Math.max(INSPECTOR_MIN, width - 16));
    else return;
    e.preventDefault();
  }
</script>

{#if open}
  <aside class="inspector" class:overlay class:dragging bind:this={panel} style="width: {width}px" aria-label="Inspector" tabindex="-1" data-inspector>
    <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions (a focusable separator is a widget) -->
    <div
      class="handle"
      role="separator"
      aria-orientation="vertical"
      aria-label="Resize inspector"
      aria-valuenow={width}
      aria-valuemin={INSPECTOR_MIN}
      aria-valuemax={INSPECTOR_MAX}
      tabindex="0"
      onpointerdown={down}
      onkeydown={key}
    ></div>
    <header class="head">
      <div class="title">
        <h2 class="t-title ellipsis" title={title}>{title}</h2>
      </div>
      {@render header?.()}
      <IconButton icon="close" label="Close inspector" onclick={onclose} />
    </header>
    <div class="scroll">
      {@render children?.()}
    </div>
  </aside>
{/if}

<style>
  .inspector {
    grid-area: insp;
    position: relative;
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--surface-panel);
    border-left: var(--border-width) solid var(--border-subtle);
    z-index: var(--z-panel);
    animation: slide var(--dur-2) var(--ease-out), as-fade-in var(--dur-fade) linear;
  }
  @keyframes slide { from { transform: translateX(8px); } to { transform: none; } }
  .overlay {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    z-index: var(--z-overlay);
    box-shadow: var(--shadow-3);
  }
  .handle {
    position: absolute;
    left: -3px;
    top: 0;
    bottom: 0;
    width: 6px;
    cursor: col-resize;
    z-index: 1;
  }
  .handle:hover, .dragging .handle, .handle:focus-visible { background: var(--accent-soft); }
  .head {
    position: sticky;
    top: 0;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: var(--size-topbar);
    padding: var(--space-2) var(--space-2) var(--space-2) var(--space-4);
    border-bottom: var(--border-width) solid var(--border-subtle);
    background: var(--surface-panel);
  }
  .title { flex: 1; min-width: 0; display: flex; align-items: center; gap: var(--space-2); }
  .scroll { flex: 1; min-height: 0; overflow: auto; }
</style>
