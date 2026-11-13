<script lang="ts">
  // Count chip. A state tone draws the state's soft background and text colour.
  import type { Snippet } from 'svelte';
  import type { Tone } from '../states';
  import type { IconName } from './icons';
  import Icon from './Icon.svelte';

  let {
    icon,
    label,
    tone = 'neutral',
    leading,
  }: { icon?: IconName; label: string; tone?: 'neutral' | Tone; leading?: Snippet } = $props();
</script>

<span class="chip st-{tone}" class:neutral={tone === 'neutral'}>
  {#if leading}{@render leading()}{:else if icon}<Icon name={icon} size={14} />{/if}
  <span class="num">{label}</span>
</span>

<style>
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 22px;
    padding: 0 var(--space-2);
    border-radius: var(--radius-pill);
    font-size: var(--text-xs);
    background: var(--st-soft);
    color: var(--st-text);
    white-space: nowrap;
  }
  .neutral {
    background: var(--surface-sunken);
    color: var(--text-secondary);
  }
</style>
