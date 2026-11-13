<script lang="ts">
  // A tiny mock of a node on a canvas, drawn in the given theme and mode (ux-spec 2.8).
  // The tile sets data-theme/data-mode on itself, so the theme's tokens apply inside it.
  import { THEME_LABELS, type ThemeName } from '../theme/theme';

  let {
    theme,
    mode,
    selected,
    onselect,
  }: { theme: ThemeName; mode: 'light' | 'dark'; selected: boolean; onselect: () => void } = $props();
</script>

<button type="button" class="tile" class:selected role="radio" aria-checked={selected} onclick={onselect}>
  <span class="scope" data-theme={theme} data-mode={mode}>
    <span class="canvas canvas-surface">
      <span class="node">
        <span class="bar"></span>
        <span class="lines">
          <span class="title">Aa</span>
          <span class="line"></span>
          <span class="chips"><span class="c st-running"></span><span class="c st-done"></span><span class="c st-error"></span></span>
        </span>
      </span>
    </span>
  </span>
  <span class="name">{THEME_LABELS[theme]}</span>
</button>

<style>
  .tile { display: flex; flex-direction: column; gap: var(--space-2); padding: var(--space-1); border-radius: var(--radius-md); text-align: left; }
  .tile:hover .canvas { box-shadow: inset 0 0 0 1px var(--border-strong); }
  .selected .scope { outline: 2px solid var(--select-ring); outline-offset: 2px; }
  .scope { display: block; border-radius: var(--radius-md); }
  /* Inside .scope the theme's own tokens apply, because the theme selectors match [data-theme]. */
  .canvas {
    display: grid;
    place-items: center;
    height: 84px;
    border-radius: var(--radius-md);
    border: var(--border-width) solid var(--border-default);
    color: var(--text-primary);
    overflow: hidden;
  }
  .node {
    display: flex;
    width: 78%;
    height: 46px;
    background: var(--surface-card);
    border: var(--border-width) solid var(--state-running);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-1);
    overflow: hidden;
  }
  .bar { width: 4px; background: var(--state-running); }
  .lines { flex: 1; display: flex; flex-direction: column; gap: 4px; padding: 5px 7px; min-width: 0; }
  .title { font-family: var(--font-display); font-weight: var(--display-weight); font-size: 12px; line-height: 14px; color: var(--text-primary); }
  .line { height: 4px; width: 70%; border-radius: var(--radius-pill); background: var(--border-default); }
  .chips { display: flex; gap: 3px; }
  .c { width: 10px; height: 6px; border-radius: var(--radius-xs); background: var(--st); }
  .name { font-size: var(--text-sm); font-weight: var(--weight-medium); color: var(--text-primary); padding-left: 2px; }
</style>
