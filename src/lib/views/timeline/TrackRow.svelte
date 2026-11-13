<script lang="ts">
  // One row's track (ux-spec 3.4): agent bar with waiting, running and finished segments, merged
  // tool ticks as a single path, retry notch, duration label; or a span bar for run and phase.
  import type { Agent } from '../../contract';
  import { formatDuration } from '../../format';
  import { AGENT_STATE } from '../../states';
  import { tickPath, toX, type Scale } from './timeScale';

  let {
    agent,
    span,
    kind,
    ticks,
    scale,
    width,
    height,
    viewTime,
    selected = false,
  }: {
    agent?: Agent | null;
    span?: { start: number | null; end: number | null };
    kind: 'agent' | 'session' | 'run' | 'phase';
    ticks: readonly number[];
    scale: Scale;
    width: number;
    height: number;
    viewTime: number;
    selected?: boolean;
  } = $props();

  const BAR = 14;
  const y = $derived((height - BAR) / 2);
  const clampX = (x: number) => Math.max(-4, Math.min(width + 4, x));

  const bar = $derived.by(() => {
    if (kind !== 'agent' || !agent) return null;
    const a = agent;
    const out: { wait?: [number, number]; body?: [number, number]; cap?: number; tone: string; finished: boolean; label?: string } = {
      tone: AGENT_STATE[a.state].tone,
      finished: a.endedAt != null && (a.state === 'done' || a.state === 'failed' || a.state === 'stopped'),
    };
    const q = a.queuedAt;
    const s = a.startedAt;
    if (q != null && (s == null || s > q)) out.wait = [toX(scale, q), toX(scale, s ?? viewTime)];
    if (s != null) {
      const e = a.endedAt ?? viewTime;
      const x0 = toX(scale, s);
      const x1 = Math.max(x0 + 2, toX(scale, Math.max(s, e)));
      out.body = [x0, x1];
      if (out.finished) out.cap = x1;
      if (x1 - x0 >= 64) out.label = formatDuration(Math.max(0, e - s));
    }
    return out;
  });

  const spanBar = $derived.by(() => {
    if ((kind !== 'run' && kind !== 'phase') || !span || span.start == null) return null;
    const x0 = toX(scale, span.start);
    const x1 = Math.max(x0 + 2, toX(scale, span.end ?? viewTime));
    return [x0, x1] as const;
  });

  const path = $derived(ticks.length ? tickPath(ticks, scale, y, BAR, width) : '');
</script>

<svg class="track" {width} {height} aria-hidden="true">
  {#if bar}
    {#if bar.wait}
      {@const [w0, w1] = bar.wait}
      {#if w1 > -4 && w0 < width + 4}
        <line class="wait" x1={clampX(w0)} x2={clampX(w1)} y1={height / 2} y2={height / 2} />
      {/if}
    {/if}
    {#if bar.body}
      {@const [b0, b1] = bar.body}
      {#if b1 > -4 && b0 < width + 4}
        <rect class="body st-{bar.tone}" class:finished={bar.finished} class:failed={agent?.state === 'failed'} class:sel={selected} x={clampX(b0)} y={y} width={Math.max(2, clampX(b1) - clampX(b0))} height={BAR} rx="2" />
        {#if bar.cap != null && bar.cap > 0 && bar.cap < width + 4}
          <rect class="cap st-{bar.tone}" x={bar.cap - 3} y={y} width="3" height={BAR} />
        {/if}
        {#if agent && agent.attempt != null && agent.attempt > 1 && b0 >= 0}
          <path class="notch" d="M{b0 - 4} {y - 1}h8l-4 5z" />
        {/if}
        {#if bar.label && b1 - Math.max(b0, 0) >= 64}
          <text class="dur num" x={Math.max(b0, 0) + 6} y={y + BAR - 3.5}>{bar.label}</text>
        {/if}
      {/if}
    {/if}
  {/if}
  {#if spanBar}
    <rect class="span {kind}" x={clampX(spanBar[0])} y={height / 2 - 3} width={Math.max(2, clampX(spanBar[1]) - clampX(spanBar[0]))} height="6" rx="2" />
  {/if}
  {#if path}<path class="ticks" d={path} />{/if}
</svg>

<style>
  .track { display: block; overflow: hidden; }
  .wait { stroke: var(--state-queued); stroke-width: 4; stroke-dasharray: 4 3; }
  .body { fill: var(--st); }
  .body.finished { fill-opacity: 0.55; }
  .body.failed { fill: var(--state-error-soft); fill-opacity: 1; stroke: var(--state-error); stroke-width: 1; }
  .body.sel { stroke: var(--select-ring); stroke-width: 2; }
  .cap { fill: var(--st); }
  .notch { fill: var(--text-secondary); }
  .dur { fill: var(--text-primary); font-size: var(--text-xs); font-family: var(--font-ui); pointer-events: none; }
  .span.run { fill: var(--border-strong); }
  .span.phase { fill: var(--accent-soft); stroke: var(--accent); stroke-width: 1; }
  .ticks { fill: var(--text-primary); fill-opacity: 0.45; }
</style>
