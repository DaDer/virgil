<script lang="ts">
  // The one inline SVG sprite (ux-spec 6.1 Icon). Mounted once by the app frame.
  import { ICONS } from './icons';
  const markup = Object.entries(ICONS)
    .map(([name, body]) => `<symbol id="i-${name}" viewBox="0 0 24 24">${body}</symbol>`)
    .join('');
</script>

<svg class="sprite" aria-hidden="true" xmlns="http://www.w3.org/2000/svg">
  <!-- eslint-disable-next-line svelte/no-at-html-tags -- static, bundled markup -->
  {@html markup}
</svg>

<style>
  .sprite { position: absolute; width: 0; height: 0; overflow: hidden; }
</style>
