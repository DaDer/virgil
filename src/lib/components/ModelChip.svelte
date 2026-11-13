<script lang="ts">
  // Model chip (ux-spec 1.3): a 14 px square with the family letter, then the short name.
  import { MODEL_LETTER, modelFamily, shortModel } from '../format';

  let { model, compact = false }: { model: string | null; compact?: boolean } = $props();
  const fam = $derived(modelFamily(model));
  const name = $derived(shortModel(model));
</script>

<span class="chip" title={model ?? 'Unknown model'}>
  <span class="sq" style="--m: var(--model-{fam})" aria-hidden="true">{MODEL_LETTER[fam]}</span>
  {#if compact}<span class="sr-only">{name}</span>{:else}<span class="name ellipsis">{name}</span>{/if}
</span>

<style>
  .chip { display: inline-flex; align-items: center; gap: 6px; min-width: 0; font-size: var(--text-sm); color: var(--text-secondary); }
  .sq {
    flex: none;
    display: inline-grid;
    place-items: center;
    width: 14px;
    height: 14px;
    border-radius: var(--radius-xs);
    background: var(--m);
    color: var(--surface-card);
    font-size: 10px;
    line-height: 1;
    font-weight: var(--weight-bold);
    font-family: var(--font-ui);
  }
</style>
