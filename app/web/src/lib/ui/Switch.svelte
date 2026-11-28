<script lang="ts">
  // Toggle: a checkbox drawn as a switch. Give it a label: `aria-label`, or a <label> around it.
  import type { HTMLInputAttributes } from 'svelte/elements';

  let { checked = $bindable(false), ...rest }: Omit<HTMLInputAttributes, 'type'> = $props();
</script>

<input type="checkbox" role="switch" class="switch" bind:checked {...rest} />

<style>
  .switch {
    appearance: none; flex: none; position: relative; margin: 0;
    width: var(--space-8); height: var(--space-5); border-radius: var(--radius-capsule);
    background: var(--color-fill); box-shadow: inset 0 0 0 var(--border-width) var(--color-control-border);
    transition: background var(--duration-fast) var(--easing);
  }
  .switch::after {
    content: ''; position: absolute; top: 2px; left: 2px; width: var(--space-4); height: var(--space-4);
    border-radius: var(--radius-capsule); background: var(--color-on-accent); box-shadow: var(--shadow-card);
    transition: left var(--duration-fast) var(--easing);
  }
  .switch:checked { background: var(--color-accent); box-shadow: none; }
  .switch:checked::after { left: calc(100% - var(--space-4) - 2px); }
</style>
