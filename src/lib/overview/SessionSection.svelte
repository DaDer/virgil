<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { Session } from '../contract';
  import Button from '../components/Button.svelte';

  let {
    title,
    sessions,
    limit,
    card,
  }: { title: string; sessions: Session[]; limit?: number; card: Snippet<[Session]> } = $props();

  let showAll = $state(false);
  const shown = $derived(limit && !showAll ? sessions.slice(0, limit) : sessions);
</script>

<section class="section" aria-label={title}>
  <h2 class="t-label head">{title} <span class="count num">{sessions.length}</span></h2>
  <div class="grid" class:single={sessions.length === 1}>
    {#each shown as s (s.sessionId)}
      {@render card(s)}
    {/each}
  </div>
  {#if limit && sessions.length > limit && !showAll}
    <div class="more"><Button variant="ghost" onclick={() => (showAll = true)}>Show older ({sessions.length - limit})</Button></div>
  {/if}
</section>

<style>
  .section { display: flex; flex-direction: column; gap: var(--space-3); }
  .head { display: flex; align-items: center; gap: var(--space-2); color: var(--text-secondary); }
  .count {
    padding: 0 6px;
    border-radius: var(--radius-pill);
    background: var(--surface-sunken);
    color: var(--text-secondary);
    font-size: var(--text-xs);
  }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(320px, 1fr)); gap: var(--space-4); }
  .grid.single { grid-template-columns: minmax(320px, 480px); }
  .more { display: flex; justify-content: center; }
</style>
