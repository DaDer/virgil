<script lang="ts">
  // ux-spec 2.10: a 48 px line icon, a title, one sentence, at most one button.
  import type { Snippet } from 'svelte';
  import Button from './Button.svelte';
  import Icon from './Icon.svelte';
  import type { IconName } from './icons';

  let {
    icon = 'empty',
    title,
    text,
    action,
    children,
    tone,
  }: {
    icon?: IconName;
    title: string;
    text?: string;
    action?: { label: string; onclick: () => void; icon?: IconName };
    children?: Snippet;
    tone?: 'error';
  } = $props();
</script>

<div class="empty" class:error={tone === 'error'} role="status">
  <span class="icon"><Icon name={icon} size={48} /></span>
  <h2 class="t-title title">{title}</h2>
  {#if text}<p class="text">{text}</p>{/if}
  {@render children?.()}
  {#if action}
    <Button variant="primary" icon={action.icon} onclick={action.onclick}>{action.label}</Button>
  {/if}
</div>

<style>
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-2);
    max-width: 420px;
    margin: auto;
    padding: var(--space-8) var(--space-4);
    text-align: center;
  }
  .icon { color: var(--text-muted); margin-bottom: var(--space-2); }
  .icon :global(.icon) { stroke-width: 1; }
  .error .icon { color: var(--state-error); }
  .title { font-size: var(--text-lg); }
  .text { color: var(--text-secondary); margin-bottom: var(--space-3); user-select: text; overflow-wrap: anywhere; }
</style>
