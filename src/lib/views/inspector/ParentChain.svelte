<script lang="ts">
  // `Session › Run › Phase › Agent`; every part selects that thing.
  import type { ChainPart } from '../../data/derive';
  import { parsePhaseKey } from '../../data/derive';
  import { actions } from '../../stores/actions';
  import { sel } from '../../stores/selection.svelte';

  let { chain, sessionId }: { chain: ChainPart[]; sessionId: string } = $props();

  function pick(p: ChainPart) {
    if (p.kind === 'session') return actions.select(sel.session(sessionId));
    if (p.kind === 'agent') return actions.select(sel.agent(sessionId, p.id));
    if (p.kind === 'run') return actions.select(sel.run(sessionId, p.id));
    const k = parsePhaseKey(p.id);
    if (k) actions.select(sel.phase(sessionId, k.runId, k.index));
  }
</script>

<nav class="chain t-small" aria-label="Parent chain">
  {#each chain as p, i (p.kind + p.id)}
    {#if i > 0}<span class="sep" aria-hidden="true">›</span>{/if}
    <button type="button" class="part ellipsis" class:last={i === chain.length - 1} title={p.label} onclick={() => pick(p)}>{p.label}</button>
  {/each}
</nav>

<style>
  .chain { display: flex; flex-wrap: wrap; align-items: center; gap: 2px 4px; padding: var(--space-3) var(--space-4) 0; }
  .sep { color: var(--text-muted); }
  .part { max-width: 160px; padding: 0 4px; border-radius: var(--radius-xs); color: var(--accent-text); }
  .part:hover { background: var(--surface-hover); text-decoration: underline; }
  .part.last { color: var(--text-primary); font-weight: var(--weight-semibold); }
</style>
