<script lang="ts">
  // Card: a group of rows or content on its own surface (System Settings' grouped rows). Put
  // CardRow inside for the rows, which it divides with a line.
  import type { Snippet } from 'svelte';
  import type { HTMLAttributes } from 'svelte/elements';

  let { children, ...rest }: HTMLAttributes<HTMLDivElement> & { children: Snippet } = $props();
</script>

<div class="card" {...rest}>{@render children()}</div>

<style>
  .card {
    border: var(--border-width) solid var(--color-separator); border-radius: var(--radius-card);
    background: var(--color-card-bg);
  }
  .card > :global(.card-row + .card-row) { border-top: var(--border-width) solid var(--color-separator); }
</style>
