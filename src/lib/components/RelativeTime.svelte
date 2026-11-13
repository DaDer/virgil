<script lang="ts">
  import { formatClock, formatRelative } from '../format';
  import { clock } from '../stores/clock.svelte';

  let { t, prefix = '' }: { t: number | null | undefined; prefix?: string } = $props();
</script>

<time class="num" datetime={t != null ? new Date(t).toISOString() : undefined} title={t != null ? formatClock(t) : undefined}>
  {prefix}{formatRelative(t, clock.viewTime)}
</time>
