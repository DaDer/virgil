<script lang="ts">
  // Tool calls by tool name, top 4 then "other", as small bars.
  let { counts }: { counts: { name: string; n: number }[] } = $props();
  const max = $derived(Math.max(1, ...counts.map((c) => c.n)));
</script>

<ul class="bd">
  {#each counts as c (c.name)}
    <li class="row">
      <span class="name t-mono">{c.name}</span>
      <span class="bar"><span style="transform: scaleX({c.n / max})"></span></span>
      <span class="n num">{c.n}</span>
    </li>
  {/each}
</ul>

<style>
  .bd { display: flex; flex-direction: column; gap: 3px; }
  .row { display: grid; grid-template-columns: 96px 1fr 36px; align-items: center; gap: var(--space-2); font-size: var(--text-xs); }
  .name { font-size: var(--text-xs); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .bar { height: 6px; border-radius: var(--radius-pill); background: var(--surface-sunken); overflow: hidden; }
  .bar span { display: block; height: 100%; background: var(--text-muted); transform-origin: left; transition: transform var(--dur-2) var(--ease-out); }
  .n { text-align: right; color: var(--text-secondary); }
</style>
