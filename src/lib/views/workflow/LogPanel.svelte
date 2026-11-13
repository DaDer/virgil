<script lang="ts">
  // ux-spec 3.3 LogPanel: a bottom drawer, 160 px, collapsible to a 32 px bar with the count and
  // the newest line. Sticks to the bottom while the user has not scrolled up.
  import Icon from '../../components/Icon.svelte';
  import { formatClock } from '../../format';
  import type { ActivityLine } from './model';

  let { lines, open, ontoggle, note }: { lines: ActivityLine[]; open: boolean; ontoggle: () => void; note?: string } = $props();

  let box: HTMLDivElement | undefined = $state();
  let stick = true;
  $effect(() => {
    void lines.length;
    if (open && box && stick) queueMicrotask(() => box && (box.scrollTop = box.scrollHeight));
  });
  const newest = $derived(lines.at(-1));
</script>

<section class="log" class:open aria-label="Run activity">
  <button type="button" class="bar" onclick={ontoggle} aria-expanded={open}>
    <span class="t-label">Activity</span>
    <span class="count num t-small">({lines.length})</span>
    {#if !open && newest}<span class="newest t-mono ellipsis">{formatClock(newest.t)} {newest.text}</span>{/if}
    <span class="flex"></span>
    <Icon name={open ? 'chevron-down' : 'chevron-right'} size={14} />
  </button>
  {#if open}
    <div class="lines t-mono" bind:this={box} onscroll={() => box && (stick = box.scrollHeight - box.scrollTop - box.clientHeight < 8)}>
      {#each lines as l, i (i)}
        <p class="ln st-{l.tone}"><span class="t">{formatClock(l.t)}</span> <span class="dot" aria-hidden="true"></span> {l.text}</p>
      {:else}
        <p class="muted">None yet</p>
      {/each}
      {#if note}<p class="note t-small">{note}</p>{/if}
    </div>
  {/if}
</section>

<style>
  .log { flex: none; border-top: var(--border-width) solid var(--border-subtle); background: var(--surface-panel); }
  .bar { display: flex; align-items: center; gap: var(--space-2); width: 100%; height: 32px; padding: 0 var(--space-6); color: var(--text-primary); text-align: left; }
  .bar:hover { background: var(--surface-hover); }
  .count { color: var(--text-muted); }
  .newest { color: var(--text-secondary); font-size: var(--text-xs); min-width: 0; }
  .flex { flex: 1; }
  .lines { height: 128px; overflow: auto; padding: 0 var(--space-6) var(--space-2); }
  .ln { white-space: pre-wrap; overflow-wrap: anywhere; color: var(--text-primary); font-size: var(--text-xs); line-height: var(--leading-xs); }
  .t { color: var(--text-muted); }
  .dot { display: inline-block; width: 6px; height: 6px; border-radius: 50%; background: var(--st); vertical-align: middle; }
  .note { color: var(--text-muted); font-family: var(--font-ui); margin-top: var(--space-1); }
</style>
