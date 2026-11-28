<script lang="ts">
  // Button: default, primary or borderless, in three sizes (docs/DESIGN.md, "Components").
  import type { Snippet } from 'svelte';
  import type { HTMLButtonAttributes } from 'svelte/elements';

  let {
    variant = 'default',
    size = 'regular',
    type = 'button',
    ref = $bindable(),
    children,
    ...rest
  }: HTMLButtonAttributes & {
    variant?: 'default' | 'primary' | 'borderless';
    size?: 'small' | 'regular' | 'large';
    /** The button element, for focus. */
    ref?: HTMLButtonElement;
    children: Snippet;
  } = $props();
</script>

<button bind:this={ref} {type} class="button {variant} {size}" {...rest}>{@render children()}</button>

<style>
  .button {
    display: inline-flex; align-items: center; justify-content: center; gap: var(--space-1);
    height: var(--control-height); padding: 0 var(--control-padding-x);
    border: var(--border-width) solid var(--color-control-border); border-radius: var(--radius-control);
    background: var(--color-control-bg); color: var(--color-label);
    font: inherit; font-size: var(--text-body); white-space: nowrap; cursor: default;
  }
  .button:active { background: var(--color-pressed); }
  .button:disabled { color: var(--color-label-tertiary); }
  .primary { background: var(--color-accent); border-color: var(--color-accent); color: var(--color-on-accent); }
  .primary:active { filter: brightness(0.92); }
  .borderless { border-color: transparent; background: transparent; }
  .borderless:hover { background: var(--color-hover); }
  .small { height: var(--control-height-small); font-size: var(--text-callout); }
  .large { height: var(--control-height-large); padding: 0 var(--control-padding-x-large); }
</style>
