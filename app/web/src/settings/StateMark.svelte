<script lang="ts">
  // A state line beside a setting: "Key saved" in green, "No key" in red, "Testing…" in orange.
  import type { Mark } from './state.svelte';

  let { mark, small = false, id }: { mark: Mark | null | undefined; small?: boolean; id?: string } = $props();
</script>

{#if small}
  <small class="state {mark?.tone ?? ''}" {id}>{mark?.text ?? ''}</small>
{:else}
  <span class="state {mark?.tone ?? ''}" {id}>{mark?.text ?? ''}</span>
{/if}
