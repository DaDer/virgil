<script lang="ts">
  // One small square per phase, filled by the phase's state (ux-spec 3.2 RunGroup header).
  import type { PhaseState } from '../contract';
  import { PHASE_STATE } from '../states';

  let {
    phases,
    current,
    size = 10,
  }: { phases: { title: string; state: PhaseState; done: number; total: number }[]; current?: number; size?: number } = $props();
</script>

<ol class="stepper" aria-label="Phases">
  {#each phases as p, i (i)}
    {@const meta = PHASE_STATE[p.state]}
    <li
      class="step st-{meta.tone}"
      class:filled={p.state !== 'pending'}
      class:current={current === i + 1}
      style="width: {size}px; height: {size}px"
      title="{i + 1} {p.title}: {meta.label}, {p.done}/{p.total}"
    >
      <span class="sr-only">{i + 1} {p.title}: {meta.label}, {p.done} of {p.total} done</span>
    </li>
  {/each}
</ol>

<style>
  .stepper { display: inline-flex; align-items: center; gap: 3px; }
  .step {
    border-radius: var(--radius-xs);
    border: var(--border-width) solid var(--border-strong);
    transition: background-color var(--dur-2) var(--ease-out), border-color var(--dur-2) var(--ease-out);
  }
  .filled { background: var(--st); border-color: var(--st); }
  .current { outline: 1.5px solid var(--st); outline-offset: 1.5px; }
</style>
