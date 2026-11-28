<script lang="ts">
  // Connector account rows: a title, its state in its tone, and a chevron; the whole row opens.
  import type { ConnectorRow } from '../../lib/ipc';
  import { tone } from '../state.svelte';

  let { rows, words, onpick }: { rows: ConnectorRow[]; words: string; onpick: (row: ConnectorRow) => void } = $props();
</script>

{#each rows as r (r.id)}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions (as the old page: the row's state links are reachable from the list) -->
  <div class="row" data-words="{words} {r.title}" style="cursor:pointer" onclick={() => onpick(r)}>
    <div class="what">{r.title}</div>
    <span><span class="state {tone(r.tone)}">{r.state}</span> ›</span>
  </div>
{/each}
