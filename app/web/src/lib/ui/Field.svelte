<script lang="ts">
  // Field: a text, number, password, search or URL field, or with `multiline`, a text area.
  import type { HTMLInputAttributes, HTMLTextareaAttributes } from 'svelte/elements';

  type Props =
    | ({ multiline?: false; value?: string | number | null } & Omit<HTMLInputAttributes, 'value'>)
    | ({ multiline: true; value?: string | null } & Omit<HTMLTextareaAttributes, 'value'>);

  let { value = $bindable(''), multiline = false, ...rest }: Props = $props();
</script>

{#if multiline}
  <textarea class="field" bind:value {...rest as HTMLTextareaAttributes}></textarea>
{:else}
  <input class="field" type="text" bind:value {...rest as HTMLInputAttributes} />
{/if}

<style>
  .field {
    height: var(--control-height); padding: 0 var(--field-padding-x);
    border: var(--border-width) solid var(--color-control-border); border-radius: var(--radius-control);
    background: var(--color-control-bg); color: var(--color-label);
    font: inherit; font-size: var(--text-body); min-width: 0;
    -webkit-user-select: text; user-select: text;
  }
  .field::placeholder { color: var(--color-label-tertiary); }
  .field:focus { outline: none; border-color: var(--color-accent); box-shadow: 0 0 0 var(--focus-ring-width) var(--color-focus-ring); }
  textarea.field { height: auto; padding: var(--space-1) var(--field-padding-x); resize: vertical; }
</style>
