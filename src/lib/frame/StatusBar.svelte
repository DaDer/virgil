<script lang="ts">
  // ux-spec 2.4: data source sentence on the left; counts, last update and version on the right.
  import type { SourceStatus } from '../contract';
  import { formatDate, formatRelative } from '../format';
  import { APP_NAME, APP_VERSION } from '../version';
  import type { Problem } from '../contract';
  import ProblemsPopover from './ProblemsPopover.svelte';

  let {
    source,
    sourceKind,
    busySessions,
    runningAgents,
    lastEventAt,
    problems,
    now,
    onchoosefolder,
  }: {
    source: SourceStatus | null;
    sourceKind: 'tauri' | 'mock' | null;
    busySessions: number;
    runningAgents: number;
    lastEventAt: number | null;
    problems: Problem[];
    /** Wall clock, for "Last update 2s ago". */
    now: number;
    onchoosefolder?: () => void;
  } = $props();

  let problemsBtn: HTMLButtonElement | undefined = $state();
  let problemsOpen = $state(false);

  const line = $derived.by(() => {
    if (!source) return { tone: 'idle', text: 'Connecting…', pulse: false };
    if (source.mode === 'replay') {
      const r = source.recording;
      const when = r ? ` · recorded ${formatDate(r.startedAt)}` : '';
      const playing = source.replay?.playing;
      return {
        tone: playing ? 'running' : 'idle',
        text: `Replaying ${r?.name ?? 'recording'}${when}${playing ? ` · ${source.replay?.speed}x` : ''}${r && !r.complete ? ' · incomplete recording' : ''}`,
        pulse: false,
      };
    }
    if (source.error) return { tone: 'error', text: source.error, pulse: false, folder: true };
    if (problems.length) return { tone: 'queued', text: `Some files could not be read (${problems.length})`, pulse: false, problems: true };
    return { tone: 'running', text: `Watching ${source.claudeDir ?? 'the Claude folder'}${sourceKind === 'mock' ? ' (demo data)' : ''}`, pulse: true };
  });

  const stale = $derived(lastEventAt != null && runningAgents > 0 && now - lastEventAt > 30_000);
</script>

<footer class="status" class:replay={source?.mode === 'replay'}>
  <div class="left">
    <span class="dot st-{line.tone}" class:loops={line.pulse} class:pulse={line.pulse} aria-hidden="true"></span>
    <span class="ellipsis" title={line.text}>{line.text}</span>
    {#if 'folder' in line && line.folder && onchoosefolder}
      <button type="button" class="link" onclick={onchoosefolder}>Choose folder</button>
    {/if}
    {#if 'problems' in line && line.problems}
      <button type="button" class="link" bind:this={problemsBtn} onclick={() => (problemsOpen = !problemsOpen)}>Show problems</button>
    {/if}
  </div>
  <div class="right num">
    <span>{busySessions} {busySessions === 1 ? 'session' : 'sessions'} busy · {runningAgents} {runningAgents === 1 ? 'agent' : 'agents'} running</span>
    {#if lastEventAt != null}<span class:late={stale}>Last update {formatRelative(lastEventAt, now)}</span>{/if}
    <span class="muted">{APP_NAME} {APP_VERSION}</span>
  </div>
</footer>

<ProblemsPopover {problems} anchor={problemsBtn} open={problemsOpen} onclose={() => (problemsOpen = false)} />

<style>
  .status {
    grid-area: status;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
    height: var(--size-statusbar);
    padding: 0 var(--space-3);
    background: var(--surface-panel);
    border-top: var(--border-width) solid var(--border-subtle);
    color: var(--text-secondary);
    font-size: var(--text-sm);
    line-height: 1;
    min-width: 0;
    z-index: var(--z-panel);
  }
  .left, .right { display: flex; align-items: center; gap: var(--space-3); min-width: 0; }
  .right { flex: none; }
  .dot { flex: none; width: 8px; height: 8px; border-radius: 50%; background: var(--st); }
  .pulse { animation: as-blink calc(var(--dur-loop) * 2) var(--ease-in-out) infinite alternate; }
  .link { color: var(--accent-text); text-decoration: underline; text-underline-offset: 2px; white-space: nowrap; }
  .late { color: var(--state-queued-text); }
</style>
