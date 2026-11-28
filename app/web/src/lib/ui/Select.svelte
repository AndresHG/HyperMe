<script lang="ts" generics="T extends string | number">
  // The pop-up button: one choice from a short list.
  import type { HTMLSelectAttributes } from 'svelte/elements';

  let {
    value = $bindable(),
    options,
    ...rest
  }: Omit<HTMLSelectAttributes, 'value'> & { value?: T; options: { value: T; label: string }[] } = $props();
</script>

<select class="select" bind:value {...rest}>
  {#each options as o (o.value)}<option value={o.value}>{o.label}</option>{/each}
</select>

<style>
  .select {
    height: var(--control-height); padding: 0 var(--field-padding-x);
    border: var(--border-width) solid var(--color-control-border); border-radius: var(--radius-control);
    background: var(--color-control-bg); color: var(--color-label);
    font: inherit; font-size: var(--text-body); min-width: 0;
  }
  .select:focus { outline: none; border-color: var(--color-accent); box-shadow: 0 0 0 var(--focus-ring-width) var(--color-focus-ring); }
</style>
