<script lang="ts">
  // Long text clamped to `lines`, with Show all / Show less and Copy. Line breaks are kept.
  import Button from './Button.svelte';
  import { copyText } from './clipboard';

  let { text, lines = 12, mono = false, copy = true }: { text: string; lines?: number; mono?: boolean; copy?: boolean } = $props();

  let expanded = $state(false);
  let overflowing = $state(false);
  let el: HTMLDivElement | undefined = $state();
  let copied = $state(false);

  $effect(() => {
    void text;
    void lines;
    if (!el) return;
    queueMicrotask(() => {
      if (el) overflowing = el.scrollHeight > el.clientHeight + 1 || expanded;
    });
  });

  async function doCopy() {
    if (await copyText(text)) {
      copied = true;
      setTimeout(() => (copied = false), 1200);
    }
  }
</script>

<div class="clamp">
  <div class="text" class:t-mono={mono} class:open={expanded} style="--lines: {lines}" bind:this={el}>{text}</div>
  {#if overflowing || copy}
    <div class="actions">
      {#if overflowing}
        <Button variant="ghost" size="sm" onclick={() => (expanded = !expanded)}>{expanded ? 'Show less' : 'Show all'}</Button>
      {/if}
      {#if copy}
        <Button variant="ghost" size="sm" icon="copy" onclick={doCopy}>{copied ? 'Copied' : 'Copy'}</Button>
      {/if}
    </div>
  {/if}
</div>

<style>
  .text {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: var(--lines);
    line-clamp: var(--lines);
    overflow: hidden;
  }
  .text.open { display: block; -webkit-line-clamp: unset; line-clamp: none; }
  .actions { display: flex; gap: var(--space-1); margin-top: var(--space-1); margin-left: calc(-1 * var(--space-2)); }
</style>
