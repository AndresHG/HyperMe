<script lang="ts">
  // One row of a Card: what it is on the left, its control on the right.
  import type { Snippet } from 'svelte';
  import type { HTMLAttributes } from 'svelte/elements';

  let { children, ...rest }: HTMLAttributes<HTMLDivElement> & { children: Snippet } = $props();
</script>

<div class="card-row" {...rest}>{@render children()}</div>

<style>
  .card-row {
    display: flex; align-items: center; justify-content: space-between; gap: var(--space-4);
    padding: var(--space-3) var(--space-4);
  }
</style>
