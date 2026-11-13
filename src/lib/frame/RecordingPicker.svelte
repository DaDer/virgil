<script lang="ts">
  // ux-spec 2.6: recordings found by the core plus recently opened ones, a native "Browse…" folder
  // dialog (Tauri only; opened from Rust) and a typed folder path field.
  import type { PickedFolder, RecordingInfo } from '../contract';
  import Button from '../components/Button.svelte';
  import Dialog from '../components/Dialog.svelte';
  import Icon from '../components/Icon.svelte';
  import Skeleton from '../components/Skeleton.svelte';
  import { formatDate, formatDuration } from '../format';
  import type { RecordingRef } from '../theme/theme';

  let {
    open,
    recent,
    onopen,
    onclose,
    list,
    pick,
    error,
  }: {
    open: boolean;
    recent: RecordingRef[];
    onopen: (path: string, info?: RecordingInfo) => void;
    onclose: () => void;
    /** Loads recordings from the core's default places. */
    list: () => Promise<RecordingInfo[]>;
    /** Native folder dialog; absent outside the app. */
    pick?: () => Promise<PickedFolder>;
    error?: string | null;
  } = $props();

  let found = $state<RecordingInfo[] | null>(null);
  let loadError = $state<string | null>(null);
  let path = $state('');
  let busy = $state(false);
  /** Recordings found in folders picked with Browse…, shown first. */
  let picked = $state<RecordingInfo[]>([]);

  async function browse() {
    if (!pick) return;
    loadError = null;
    try {
      const r = await pick();
      if (!r.path) return;
      // A folder that is itself a recording opens straight away.
      if (r.recordings.length === 1 && r.recordings[0].path === r.path) {
        return openPath(r.recordings[0].path, r.recordings[0]);
      }
      if (r.recordings.length === 0) loadError = `no recordings in ${r.path}`;
      picked = [...r.recordings, ...picked.filter((p) => !r.recordings.some((x) => x.path === p.path))];
    } catch (e) {
      loadError = (e as { message?: string })?.message ?? String(e);
    }
  }

  $effect(() => {
    if (!open) return;
    found = null;
    loadError = null;
    list()
      .then((r) => (found = r))
      .catch((e) => {
        found = [];
        loadError = e?.message ?? String(e);
      });
  });

  const items = $derived.by(() => {
    const seen = new Set<string>();
    const out: { path: string; name: string; startedAt: number | null; endedAt: number | null; session: string | null; complete: boolean | null; info?: RecordingInfo }[] = [];
    for (const r of [...picked, ...(found ?? [])]) {
      if (seen.has(r.path)) continue;
      seen.add(r.path);
      out.push({ path: r.path, name: r.name, startedAt: r.startedAt, endedAt: r.endedAt, session: r.projectSlug, complete: r.complete, info: r });
    }
    for (const r of recent) {
      if (seen.has(r.path)) continue;
      out.push({ path: r.path, name: r.name, startedAt: r.startedAt, endedAt: r.endedAt, session: r.sessionName, complete: null });
    }
    return out;
  });

  async function openPath(p: string, info?: RecordingInfo) {
    busy = true;
    try {
      await onopen(p, info);
    } finally {
      busy = false;
    }
  }
</script>

<Dialog {open} title="Open a recording" width={600} {onclose}>
  <div class="picker">
    {#if found === null}
      <div class="sk"><Skeleton height={44} /><Skeleton height={44} /></div>
    {:else if items.length === 0}
      <p class="muted t-small">No recordings found{loadError ? `: ${loadError}` : ''}. Recordings are folders written by tools/recorder.py.</p>
    {:else}
      <ul class="list" aria-label="Recordings">
        {#each items as r (r.path)}
          <li>
            <button type="button" class="rec" disabled={busy} onclick={() => openPath(r.path, r.info)}>
              <Icon name="replay" size={20} />
              <span class="main">
                <span class="name">{r.name}</span>
                <span class="sub t-small">
                  {r.startedAt ? formatDate(r.startedAt) : 'unknown date'}
                  {#if r.startedAt && r.endedAt} · {formatDuration(r.endedAt - r.startedAt)}{/if}
                  {#if r.session} · {r.session}{/if}
                  {#if r.complete === false} · still recording or incomplete{/if}
                </span>
              </span>
              <Icon name="chevron-right" size={16} />
            </button>
          </li>
        {/each}
      </ul>
    {/if}

    <form
      class="path"
      onsubmit={(e) => {
        e.preventDefault();
        if (path.trim()) openPath(path.trim());
      }}
    >
      <label class="t-label" for="rec-path">Open recording folder…</label>
      <div class="field">
        <Icon name="folder" size={16} />
        <input id="rec-path" bind:value={path} placeholder="/path/to/recordings/first-build" spellcheck="false" autocomplete="off" />
        <Button variant="primary" size="sm" type="submit" disabled={busy || !path.trim()}>Open</Button>
        {#if pick}
          <Button size="sm" type="button" disabled={busy} onclick={browse}>Browse…</Button>
        {/if}
      </div>
    </form>
    {#if error || (loadError && items.length > 0)}
      <p class="error t-small" role="alert"><Icon name="alert" size={14} /> {error ?? loadError}</p>
    {/if}
  </div>
</Dialog>

<style>
  .picker { display: flex; flex-direction: column; gap: var(--space-4); }
  .sk { display: flex; flex-direction: column; gap: var(--space-2); }
  .list { display: flex; flex-direction: column; gap: var(--space-1); max-height: 320px; overflow: auto; }
  .rec {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    width: 100%;
    padding: var(--space-2) var(--space-3);
    border-radius: var(--radius-md);
    border: var(--border-width) solid var(--border-subtle);
    background: var(--surface-card);
    text-align: left;
    color: var(--text-secondary);
  }
  .rec:hover:not(:disabled) { background: var(--surface-hover); border-color: var(--border-default); }
  .main { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  .name { color: var(--text-primary); font-weight: var(--weight-semibold); }
  .sub { color: var(--text-muted); }
  .path { display: flex; flex-direction: column; gap: var(--space-2); }
  .path label { color: var(--text-muted); }
  .field {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-1) 0 var(--space-2);
    height: 34px;
    border-radius: var(--radius-sm);
    border: var(--border-width) solid var(--border-default);
    background: var(--surface-sunken);
    color: var(--text-muted);
  }
  .field:focus-within { border-color: var(--focus-ring); }
  input { flex: 1; min-width: 0; border: 0; background: none; outline: none; font-family: var(--font-mono); font-size: var(--text-sm); color: var(--text-primary); }
  .error { display: flex; align-items: center; gap: var(--space-2); color: var(--state-error-text); background: var(--state-error-soft); padding: var(--space-2) var(--space-3); border-radius: var(--radius-sm); }
</style>
