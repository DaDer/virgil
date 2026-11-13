<script lang="ts">
  // Per-state segments, in order done, running, waiting, failed, stopped (ux-spec 3.2, 3.3).
  // `total` larger than the sum leaves an empty track at the end.
  import type { AgentState } from '../contract';

  let {
    counts,
    height = 4,
    animated = true,
    total,
    label,
  }: { counts: Partial<Record<AgentState, number>>; height?: number; animated?: boolean; total?: number; label?: string } = $props();

  const parts = $derived.by(() => {
    const c = counts;
    const segs = [
      { tone: 'done', n: c.done ?? 0, running: false },
      { tone: 'running', n: c.running ?? 0, running: true },
      { tone: 'queued', n: (c.waiting ?? 0) + (c.queued ?? 0), running: false },
      { tone: 'error', n: c.failed ?? 0, running: false },
      { tone: 'stopped', n: c.stopped ?? 0, running: false },
    ];
    const sum = segs.reduce((n, s) => n + s.n, 0);
    const of = Math.max(sum, total ?? 0);
    return { segs: segs.filter((s) => s.n > 0).map((s) => ({ ...s, pct: of ? (s.n / of) * 100 : 0 })), sum, of };
  });

  const aria = $derived(label ?? `${counts.done ?? 0} of ${parts.of} done`);
</script>

<div class="track" style="height: {height}px" role="img" aria-label={aria}>
  {#each parts.segs as s (s.tone)}
    <span class="seg st-{s.tone}" style="width: {s.pct}%">
      {#if s.running && animated}<span class="sheen loops"></span>{/if}
    </span>
  {/each}
</div>

<style>
  .track {
    display: flex;
    width: 100%;
    gap: 1px;
    overflow: hidden;
    border-radius: var(--radius-pill);
    background: var(--surface-sunken);
  }
  .seg {
    position: relative;
    overflow: hidden;
    background: var(--st);
    transition: width var(--dur-2) var(--ease-out), background-color var(--dur-2) var(--ease-out);
  }
  .sheen {
    position: absolute;
    inset: 0;
    background: linear-gradient(90deg, transparent, color-mix(in srgb, var(--surface-card) 55%, transparent), transparent);
    animation: as-sheen calc(var(--dur-loop) * 1.5) linear infinite;
  }
</style>
