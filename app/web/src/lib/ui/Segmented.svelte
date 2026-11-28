<script lang="ts" generics="T extends string | number">
  // Segmented control: one of two to five choices, all visible. Radio inputs, so the arrow keys
  // move between them.
  let {
    value = $bindable(),
    options,
    name,
    label,
    onchange,
  }: {
    value?: T;
    options: { value: T; label: string }[];
    /** The radio group's name, unique on the page. */
    name: string;
    /** What is being chosen, for VoiceOver. */
    label: string;
    onchange?: (value: T) => void;
  } = $props();
</script>

<div class="segmented" role="radiogroup" aria-label={label}>
  {#each options as o (o.value)}
    <label>
      <input type="radio" {name} value={o.value} bind:group={value} onchange={() => onchange?.(o.value)} />
      {o.label}
    </label>
  {/each}
</div>

<style>
  .segmented {
    display: inline-flex; padding: var(--segment-inset); gap: var(--segment-inset); border-radius: var(--radius-control);
    background: var(--color-fill);
  }
  label {
    display: flex; align-items: center; justify-content: center; gap: var(--space-1);
    height: calc(var(--control-height) - 4px); padding: 0 var(--space-3); border-radius: var(--radius-small);
    font-size: var(--text-body);
  }
  input { position: absolute; opacity: 0; pointer-events: none; }
  label:has(input:checked) { background: var(--color-control-bg); box-shadow: var(--shadow-card); font-weight: var(--weight-medium); }
  label:has(input:focus-visible) { outline: var(--focus-ring-width) solid var(--color-focus-ring); }
</style>
