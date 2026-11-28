<script lang="ts">
  // On this Mac (R-22): looks on the usual ports for a model server and offers what answers; the
  // user may type another address instead.
  import { onMount } from 'svelte';
  import { SvelteMap } from 'svelte/reactivity';
  import { commands, type LocalServer } from '../../lib/ipc';
  import { useSettings } from '../state.svelte';

  let { oncancel }: { oncancel: () => void } = $props();
  const s = useSettings();
  let servers = $state<LocalServer[] | null>(null);
  let address = $state('');
  let model = $state('');
  /** The model picked for each server, by port. */
  const picks = new SvelteMap<number, string>();

  const status = $derived(
    servers === null
      ? 'Looking for a server on this Mac…'
      : !servers.length
        ? 'No server answered on the usual ports. Start Ollama, LM Studio or mlx_lm.server, or type its address.'
        : servers.length === 1
          ? 'Found a server on this Mac:'
          : 'Found these servers on this Mac:',
  );

  async function use(endpoint: string, model: string) {
    try {
      await commands.localUse(endpoint, model);
      delete s.pending.cleanup;
      s.error = '';
    } catch (e) {
      s.fail(e);
    }
    await s.refresh();
  }

  function useTyped() {
    if (!address.trim() || !model.trim()) {
      s.fail('Type the server address and a model name.');
      return;
    }
    use(address, model.trim());
  }

  onMount(async () => {
    let found: LocalServer[] = [];
    try {
      found = await commands.localDiscover();
    } catch (e) {
      s.fail(e);
    }
    servers = found;
    address = found[0]?.endpoint ?? '';
  });
</script>

<div id="local-form">
  <p class="note">{status}</p>
  <div class="add">
    {#each servers ?? [] as server (server.port)}
      {#if !server.models.length}
        <span class="state warn">Port {server.port} has no models yet</span>
      {:else}
        <select
          aria-label="Model on port {server.port}"
          value={picks.get(server.port) ?? server.models[0]}
          onchange={(e) => picks.set(server.port, e.currentTarget.value)}
        >
          {#each server.models as m (m)}<option value={m}>{m}</option>{/each}
        </select>
        <button class="main" onclick={() => use(server.endpoint, picks.get(server.port) ?? server.models[0] ?? '')}>Use port {server.port}</button>
      {/if}
    {/each}
  </div>
  <div class="add">
    <input type="text" placeholder="http://127.0.0.1:11434/v1" aria-label="Server address" bind:value={address} />
    <input type="text" placeholder="Model name" aria-label="Server model" bind:value={model} />
    <button class="main" onclick={useTyped}>Use</button>
    <button onclick={oncancel}>Cancel</button>
  </div>
</div>

<style>
  .note { margin: 0 0 var(--space-2); color: var(--color-label-secondary); font-size: var(--text-callout); }
  .add { display: flex; gap: var(--space-2); margin-bottom: var(--space-2); }
  .add input { flex: 1; min-width: 0; }
</style>
