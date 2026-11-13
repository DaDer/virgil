<script lang="ts">
  // ux-spec 2.8.
  import Button from '../components/Button.svelte';
  import Dialog from '../components/Dialog.svelte';
  import SegmentedControl from '../components/SegmentedControl.svelte';
  import { actions } from '../stores/actions';
  import { settings } from '../stores/settings.svelte';
  import { ui } from '../stores/ui.svelte';
  import { world } from '../stores/world.svelte';
  import { THEMES } from '../theme/theme';
  import ThemeTile from './ThemeTile.svelte';

  let { open, onclose }: { open: boolean; onclose: () => void } = $props();

  const s = $derived(settings.value);
  const detected = $derived(world.status?.claudeDir ?? null);
  const shown = $derived(s.claudeDir ?? detected);
</script>

<Dialog {open} title="Settings" width={560} {onclose}>
  <div class="grid">
    <div class="row block">
      <span class="t-label key" id="theme-label">Theme</span>
      <div class="tiles" role="radiogroup" aria-labelledby="theme-label">
        {#each THEMES as t (t)}
          <ThemeTile theme={t} mode={settings.resolvedMode} selected={s.theme === t} onselect={() => settings.update({ theme: t })} />
        {/each}
      </div>
    </div>
    <div class="row">
      <span class="t-label key">Mode</span>
      <SegmentedControl
        label="Mode"
        value={s.mode}
        options={[{ value: 'system', label: 'System' }, { value: 'light', label: 'Light' }, { value: 'dark', label: 'Dark' }]}
        onchange={(mode) => settings.update({ mode })}
      />
    </div>
    <div class="row">
      <span class="t-label key">Density</span>
      <SegmentedControl
        label="Density"
        value={s.density}
        options={[{ value: 'comfortable', label: 'Comfortable' }, { value: 'compact', label: 'Compact' }]}
        onchange={(density) => settings.update({ density })}
      />
    </div>
    <div class="row">
      <span class="t-label key">Motion</span>
      <SegmentedControl
        label="Motion"
        value={s.motion}
        options={[{ value: 'system', label: 'System' }, { value: 'full', label: 'Full' }, { value: 'reduced', label: 'Reduced' }]}
        onchange={(motion) => settings.update({ motion })}
      />
    </div>
    <div class="row">
      <span class="t-label key">Time axis</span>
      <SegmentedControl
        label="Time axis"
        value={s.timeAxis}
        options={[{ value: 'elapsed', label: 'Elapsed' }, { value: 'clock', label: 'Clock time' }]}
        onchange={(timeAxis) => settings.update({ timeAxis })}
      />
    </div>
    <div class="row">
      <span class="t-label key">Claude folder</span>
      <div class="folder">
        <span class="t-mono path" title={shown ?? ''}>{shown ?? 'auto-detected'}</span>
        <span class="t-small hint">
          {#if !s.claudeDir}Auto-detected.{/if}
          {#if world.mode === 'replay'}Applies when you go back to Live.{/if}
        </span>
        {#if ui.folderError}<span class="t-small err" role="alert">{ui.folderError}</span>{/if}
        <div class="btns">
          <Button
            size="sm"
            disabled={!world.canChooseClaudeDir}
            title={world.canChooseClaudeDir ? 'Pick the folder Claude Code writes to' : 'Only available in the desktop app'}
            onclick={() => void actions.chooseClaudeDir()}>Choose folder…</Button
          >
          <Button size="sm" variant="ghost" disabled={!s.claudeDir} onclick={() => void actions.useClaudeDir(null)}>Reset</Button>
        </div>
      </div>
    </div>
  </div>
</Dialog>

<style>
  .grid { display: flex; flex-direction: column; gap: var(--space-4); }
  .row { display: grid; grid-template-columns: 112px 1fr; align-items: center; gap: var(--space-4); }
  .row.block { align-items: start; }
  .key { color: var(--text-muted); }
  .tiles { display: grid; grid-template-columns: repeat(4, 1fr); gap: var(--space-2); }
  .folder { display: flex; flex-direction: column; gap: var(--space-2); min-width: 0; }
  .path { overflow-wrap: anywhere; color: var(--text-secondary); }
  .btns { display: flex; gap: var(--space-2); }
  .hint { color: var(--text-muted); }
  .hint:empty { display: none; }
  .err { color: var(--state-error-text, var(--state-error)); }
</style>
