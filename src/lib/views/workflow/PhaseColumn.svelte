<script lang="ts">
  // ux-spec 3.3 PhaseColumn: header with number, title, done/total, detail, model hint and a
  // progress bar; a body of AgentCards that scrolls on its own.
  import { flip } from 'svelte/animate';
  import AgentCard from '../../components/AgentCard.svelte';
  import ModelChip from '../../components/ModelChip.svelte';
  import SegmentedProgress from '../../components/SegmentedProgress.svelte';
  import { actions } from '../../stores/actions';
  import { route } from '../../stores/route.svelte';
  import { search } from '../../stores/search.svelte';
  import { sel, selection } from '../../stores/selection.svelte';
  import { settings } from '../../stores/settings.svelte';
  import { links } from '../shared/links.svelte';
  import type { ColumnModel } from './model';

  let { col, sessionId, runId, loops }: { col: ColumnModel; sessionId: string; runId: string; loops: Set<string> } = $props();

  const selected = $derived(selection.is('phase', `${runId}:${col.phase.index}`, sessionId));
  const running = $derived(col.counts.running);
  let body: HTMLDivElement | undefined = $state();
  let scrolled = $state(false);
  const flipMs = $derived(settings.reducedMotion ? 0 : 320);

  function jumpToRunning() {
    body?.scrollTo({ top: 0, behavior: settings.reducedMotion ? 'auto' : 'smooth' });
  }
</script>

<section class="col" class:active={col.active} aria-label="Phase {col.phase.index} {col.phase.title}">
  <button type="button" class="head" class:selected onclick={() => actions.select(sel.phase(sessionId, runId, col.phase.index))}>
    <span class="line">
      <span class="n num">{col.phase.index}</span>
      <span class="title t-title ellipsis" title={col.phase.title}>{col.phase.title}</span>
      {#if col.phase.model}<ModelChip model={col.phase.model} compact />{/if}
      <span class="count num">{col.counts.done}/{col.total}</span>
    </span>
    {#if col.phase.detail}<span class="detail t-small ellipsis" title={col.phase.detail}>{col.phase.detail}</span>{/if}
    <SegmentedProgress counts={col.counts} total={col.total} height={4} animated={running > 0} />
  </button>
  <div class="body" bind:this={body} onscroll={() => (scrolled = (body?.scrollTop ?? 0) > 40)}>
    {#if scrolled && running > 0}
      <button type="button" class="jump t-small" onclick={jumpToRunning}>{running} running ↑</button>
    {/if}
    {#each col.agents as a (a.agentId)}
      <div class="slot" data-agent={a.agentId} animate:flip={{ duration: flipMs }}>
        <AgentCard
          agent={a}
          variant="card"
          detail={col.compact ? 'compact' : 'full'}
          selected={selection.is('agent', a.agentId, sessionId)}
          dimmed={!search.isMatch(sessionId, a.agentId)}
          animate={loops.has(a.agentId)}
          onclick={() => actions.select(sel.agent(sessionId, a.agentId))}
          ondblclick={() => {
            links.focusInGraph(sessionId, a.agentId);
            route.navigate('graph');
          }}
        />
      </div>
    {:else}
      <p class="empty t-small">No agents yet</p>
    {/each}
  </div>
</section>

<style>
  .col {
    flex: 0 0 288px;
    min-width: 248px;
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--surface-panel);
    border: var(--border-width) var(--container-border-style) var(--border-default);
    border-top: 3px solid var(--border-default);
    border-radius: var(--radius-lg);
    overflow: hidden;
  }
  .col.active { border-top-color: var(--state-running); }
  .head { display: flex; flex-direction: column; gap: 6px; padding: var(--pad-3); text-align: left; color: var(--text-primary); border-bottom: var(--border-width) solid var(--border-subtle); }
  .head:hover { background: var(--surface-hover); }
  .head.selected { box-shadow: inset 0 0 0 2px var(--select-ring); }
  .line { display: flex; align-items: center; gap: var(--space-2); min-width: 0; }
  .n { display: inline-grid; place-items: center; flex: none; width: 22px; height: 22px; border-radius: 50%; border: var(--border-width) solid var(--border-strong); font-size: var(--text-xs); }
  .title { flex: 1; min-width: 0; }
  .count { color: var(--text-secondary); }
  .detail { color: var(--text-secondary); }
  .body { position: relative; flex: 1; min-height: 0; overflow: auto; display: flex; flex-direction: column; gap: var(--space-2); padding: var(--pad-2); }
  .jump { position: sticky; top: 0; z-index: 1; align-self: center; padding: 2px var(--space-3); border-radius: var(--radius-pill); background: var(--state-running-soft); color: var(--state-running-text); box-shadow: var(--shadow-2); }
  .empty { color: var(--text-muted); padding: var(--space-4) var(--space-2); text-align: center; }
</style>
