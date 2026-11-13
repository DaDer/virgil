<script lang="ts">
  // A duration: `ms`, or `from` to `to` (to defaults to the clock's viewTime, so it ticks).
  import { formatDuration } from '../format';
  import { clock } from '../stores/clock.svelte';

  let { ms, from, to, allowMs = false }: { ms?: number | null; from?: number | null; to?: number | null; allowMs?: boolean } = $props();

  const value = $derived(ms != null ? ms : from != null ? Math.max(0, (to ?? clock.viewTime) - from) : null);
</script>

<span class="num">{formatDuration(value, { allowMs })}</span>
