<script lang="ts">
  import { useSettings } from './state.svelte';

  let { on }: { on: boolean } = $props();
  const s = useSettings();
</script>

<section id="about" class:on>
  <h1>About</h1>
  <p class="lead">HyperMe <span>{s.view?.version ?? ''}</span>. Licensed MIT OR Apache-2.0.</p>
</section>
