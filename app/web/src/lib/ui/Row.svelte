<script lang="ts">
  // List row: a row in a list, with hover, pressed and selected states. Selection is shown with
  // `aria-selected` (a list box) or `aria-current` (navigation), so VoiceOver hears it too.
  import type { Snippet } from 'svelte';
  import type { HTMLAttributes } from 'svelte/elements';

  let { children, ...rest }: HTMLAttributes<HTMLDivElement> & { children: Snippet } = $props();
</script>

<div class="row" {...rest}>{@render children()}</div>

<style>
  .row {
    display: flex; align-items: center; gap: var(--space-2);
    min-height: var(--row-height); padding: var(--space-1) var(--space-2); border-radius: var(--radius-control);
  }
  .row:hover { background: var(--color-hover); }
  .row:active { background: var(--color-pressed); }
  .row[aria-selected='true'], .row[aria-current='page'] { background: var(--color-accent-selected); }
</style>
