<script lang="ts">
  let { width = '100%', height = 16, radius }: { width?: number | string; height?: number | string; radius?: string } = $props();
  const css = (v: number | string) => (typeof v === 'number' ? `${v}px` : v);
</script>

<span class="sk" style="width: {css(width)}; height: {css(height)}; {radius ? `border-radius: ${radius}` : ''}" aria-hidden="true">
  <span class="shimmer loops"></span>
</span>

<style>
  .sk { position: relative; display: block; overflow: hidden; background: var(--surface-sunken); border-radius: var(--radius-sm); }
  .shimmer {
    position: absolute;
    inset: 0;
    background: linear-gradient(90deg, transparent, color-mix(in srgb, var(--surface-card) 60%, transparent), transparent);
    animation: as-shimmer 1.2s linear infinite;
  }
</style>
