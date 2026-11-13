<script lang="ts">
  // State icon (ux-spec 1.2): a distinct shape per state, coloured by the state token.
  // `state` is an agent state or any StateMeta (run, phase, session states from $lib/states).
  import type { AgentState } from '../contract';
  import { AGENT_STATE, type StateMeta } from '../states';

  let {
    state,
    size = 16,
    animated = true,
    title,
  }: { state: AgentState | StateMeta; size?: number; animated?: boolean; title?: string } = $props();

  const meta = $derived(typeof state === 'string' ? AGENT_STATE[state] : state);
</script>

<svg
  class="state-icon st-{meta.tone}"
  width={size}
  height={size}
  viewBox="0 0 16 16"
  role="img"
  aria-label={title ?? meta.label}
>
  {#if meta.icon === 'running'}
    <circle cx="8" cy="8" r="6" class="track" />
    <g class="arc" class:loops={animated} class:spin={animated}>
      <path d="M8 2a6 6 0 1 1-6 6" />
    </g>
  {:else if meta.icon === 'queued'}
    <circle cx="8" cy="8" r="6" />
    <path d="M8 4.8V8l2.2 1.4" />
  {:else if meta.icon === 'waiting'}
    <path d="M4.5 2.5h7M4.5 13.5h7" />
    <path d="M5.3 2.5c0 3 5.4 3.2 5.4 5.5s-5.4 2.5-5.4 5.5M10.7 2.5c0 3-5.4 3.2-5.4 5.5s5.4 2.5 5.4 5.5" />
  {:else if meta.icon === 'done'}
    <circle cx="8" cy="8" r="6" />
    <path d="M5.2 8.2l2 2 3.6-3.9" class="check" />
  {:else if meta.icon === 'error'}
    <path d="M8 2.2l6.2 11H1.8z" />
    <path d="M8 6.5v3" />
    <circle cx="8" cy="11.3" r="0.4" class="dot" />
  {:else if meta.icon === 'stopped'}
    <circle cx="8" cy="8" r="6" />
    <rect x="5.8" y="5.8" width="4.4" height="4.4" rx="0.6" class="fill" />
  {:else if meta.icon === 'idle'}
    <circle cx="8" cy="8" r="6" />
    <path d="M6.5 5.6v4.8M9.5 5.6v4.8" />
  {:else}
    <circle cx="8" cy="8" r="6" stroke-dasharray="2.4 2.2" />
  {/if}
</svg>

<style>
  .state-icon {
    flex: none;
    fill: none;
    stroke: var(--st);
    stroke-width: 1.5;
    stroke-linecap: round;
    stroke-linejoin: round;
    transition: stroke var(--dur-2) var(--ease-out);
  }
  .track { opacity: 0.3; }
  .arc { transform-origin: 8px 8px; }
  .spin { animation: as-spin var(--dur-loop) linear infinite; }
  .dot, .fill { fill: var(--st); stroke: none; }
  .check { stroke-dasharray: 10; stroke-dashoffset: 0; animation: draw var(--dur-2) var(--ease-out); }
  @keyframes draw { from { stroke-dashoffset: 10; } to { stroke-dashoffset: 0; } }
</style>
