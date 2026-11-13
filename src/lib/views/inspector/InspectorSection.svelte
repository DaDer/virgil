<script lang="ts">
  // A section with a small-caps title, an optional count and a collapse chevron.
  import type { Snippet } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import { sections } from './sections.svelte';

  let { title, count, defaultCollapsed = false, children }: { title: string; count?: number | string; defaultCollapsed?: boolean; children?: Snippet } = $props();
  const collapsed = $derived(sections.isCollapsed(title, defaultCollapsed));
</script>

<section class="sec">
  <button type="button" class="head" aria-expanded={!collapsed} onclick={() => sections.toggle(title, defaultCollapsed)}>
    <Icon name={collapsed ? 'chevron-right' : 'chevron-down'} size={12} />
    <span class="t-label">{title}</span>
    {#if count != null}<span class="count t-small num">{count}</span>{/if}
  </button>
  {#if !collapsed}<div class="content">{@render children?.()}</div>{/if}
</section>

<style>
  .sec { border-bottom: var(--border-width) solid var(--border-subtle); }
  .head { display: flex; align-items: center; gap: 6px; width: 100%; padding: var(--space-3) var(--space-4) var(--space-2); color: var(--text-muted); text-align: left; }
  .head:hover { color: var(--text-primary); }
  .count { color: var(--text-muted); }
  .content { padding: 0 var(--space-4) var(--space-4); display: flex; flex-direction: column; gap: var(--space-2); }
</style>
