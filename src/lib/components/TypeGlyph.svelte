<script lang="ts">
  // Agent type glyph in a tinted rounded square (ux-spec 1.3).
  import { agentTypeMeta } from '../states';
  import Icon from './Icon.svelte';

  let {
    kind = 'subagent',
    agentType = null,
    size = 20,
  }: { kind?: 'main' | 'subagent' | 'workflow' | 'session'; agentType?: string | null; size?: number } = $props();

  const meta = $derived(agentTypeMeta(agentType, kind));
</script>

<span class="glyph" style="--g: var(--type-{meta.key}); width: {size}px; height: {size}px" title={meta.label}>
  <Icon name={meta.glyph} size={Math.round(size * 0.72)} label={meta.label} />
</span>

<style>
  .glyph {
    flex: none;
    display: inline-grid;
    place-items: center;
    border-radius: var(--radius-sm);
    color: var(--g);
    background: color-mix(in srgb, var(--g) 14%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--g) 30%, transparent);
  }
  .glyph :global(.icon) { stroke-width: 2; }
</style>
