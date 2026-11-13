<script lang="ts">
  // What needs the person now: sessions that asked something, finished, went quiet, or are working,
  // each with one line on where it stands. Fed by the shell's attention list (same as the menu bar).
  import RelativeTime from '../components/RelativeTime.svelte';
  import { actions } from '../stores/actions';
  import { attention, KIND_LABEL, type AttentionItem } from '../stores/attention.svelte';
  import { world } from '../stores/world.svelte';

  function open(i: AttentionItem): void {
    void attention.markSeen(i.sessionId);
    actions.openSession(i.sessionId);
  }

  function extras(i: AttentionItem): string {
    const parts: string[] = [];
    if (i.runningAgents) parts.push(`${i.runningAgents} agent${i.runningAgents === 1 ? '' : 's'}`);
    if (i.backgroundTasks) parts.push(`${i.backgroundTasks} background command${i.backgroundTasks === 1 ? '' : 's'}`);
    return parts.join(' · ');
  }
</script>

{#if attention.available && world.status?.mode !== 'replay'}
  <section class="now" aria-label="Now">
    <header class="head">
      <h2 class="t-label title">Now</h2>
      <div class="prefs">
        <label class="pref t-small" title="Stops the computer from sleeping while any session is working">
          <input type="checkbox" checked={attention.prefs.keepAwake} onchange={(e) => attention.setPrefs({ keepAwake: e.currentTarget.checked })} />
          Keep awake while sessions work
          {#if attention.awake}<span class="on t-label">on now</span>{/if}
        </label>
        <label class="pref t-small">
          <input type="checkbox" checked={attention.prefs.notifications} onchange={(e) => attention.setPrefs({ notifications: e.currentTarget.checked })} />
          Notify me
        </label>
      </div>
    </header>

    {#if attention.groups.length === 0}
      <p class="empty t-small">Nothing is running{attention.idleCount ? ` (${attention.idleCount} idle)` : ''}.</p>
    {:else}
      <ul class="list">
        {#each attention.groups as group (group.kind)}
          {#each group.items as i (i.sessionId)}
            <li>
              <button class="row {i.kind}" type="button" onclick={() => open(i)}>
                <span class="kind t-label">
                  {#if attention.unseen.includes(i.sessionId)}<span class="dot" aria-label="New"></span>{/if}
                  {KIND_LABEL[i.kind]}
                </span>
                <span class="name">{i.name}</span>
                <span class="line t-small">{i.line}</span>
                <span class="meta t-small">
                  {#if extras(i)}<span>{extras(i)}</span>{/if}
                  <RelativeTime t={i.since} />
                </span>
              </button>
            </li>
          {/each}
        {/each}
      </ul>
    {/if}
  </section>
{/if}

<style>
  .now {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    padding: var(--space-4);
    border: var(--border-width) solid var(--border-default);
    border-radius: var(--radius-lg);
    background: var(--surface-card);
  }
  .head { display: flex; align-items: center; justify-content: space-between; gap: var(--space-4); flex-wrap: wrap; }
  .title { color: var(--text-secondary); }
  .prefs { display: flex; gap: var(--space-4); flex-wrap: wrap; }
  .pref { display: inline-flex; align-items: center; gap: var(--space-2); color: var(--text-secondary); cursor: pointer; }
  .pref input { accent-color: var(--accent); }
  .on { padding: 0 6px; border-radius: var(--radius-pill); background: var(--state-running-soft); color: var(--state-running-text); }
  .empty { color: var(--text-muted); margin: 0; }
  .list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: var(--space-1); }
  .row {
    width: 100%;
    display: grid;
    grid-template-columns: 112px minmax(96px, 180px) minmax(0, 1fr) auto;
    align-items: center;
    gap: var(--space-3);
    padding: var(--pad-2) var(--pad-3);
    border: 0;
    border-left: 3px solid var(--tone);
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-primary);
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition: background var(--dur-1) var(--ease-out);
  }
  .row:hover { background: var(--surface-hover); }
  .row:focus-visible { outline: 2px solid var(--focus-ring); outline-offset: -2px; }
  .row.needsYou { --tone: var(--state-error); --tone-text: var(--state-error-text); }
  .row.finished { --tone: var(--state-done); --tone-text: var(--state-done-text); }
  .row.quiet { --tone: var(--state-queued); --tone-text: var(--state-queued-text); }
  .row.working { --tone: var(--state-running); --tone-text: var(--state-running-text); }
  .kind { display: inline-flex; align-items: center; gap: var(--space-2); color: var(--tone-text); white-space: nowrap; }
  .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--tone); flex: none; }
  .name { font-weight: var(--weight-semibold); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .line { color: var(--text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .meta { display: inline-flex; gap: var(--space-3); color: var(--text-muted); white-space: nowrap; }
  @media (max-width: 1100px) {
    .row { grid-template-columns: 104px minmax(0, 1fr) auto; }
    .line { grid-column: 1 / -1; grid-row: 2; }
  }
</style>
