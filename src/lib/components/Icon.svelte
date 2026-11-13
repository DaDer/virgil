<script lang="ts">
  import type { IconName } from './icons';

  let { name, size = 16, label, class: cls = '' }: { name: IconName; size?: number; label?: string; class?: string } = $props();
</script>

<svg
  class="icon {cls}"
  width={size}
  height={size}
  role={label ? 'img' : undefined}
  aria-label={label}
  aria-hidden={label ? undefined : 'true'}
>
  <use href="#i-{name}" />
</svg>

<style>
  .icon {
    flex: none;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.5;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
</style>
