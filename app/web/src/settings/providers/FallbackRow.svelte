<script lang="ts">
  // A role's fallback (specs/96, R-24 to R-24a): one per role, saved only once its Test passes. A
  // fallback provider with no key yet takes it here (one key per provider, D-2).
  import { commands, type RoleView } from '../../lib/ipc';
  import State from '../State.svelte';
  import { mark, testing, useSettings, type Mark } from '../state.svelte';

  let { r }: { r: RoleView } = $props();
  const s = useSettings();

  let provider = $state('');
  let model = $state('');
  let key = $state('');
  let changed = $state<Mark | null>(null);
  /** What the last passing Test saved, shown on the row Rust draws next. */
  let saved = $state<{ provider: string; model: string; text: string } | null>(null);
  $effect.pre(() => {
    provider = r.fallback?.provider ?? '';
    model = r.fallback?.model ?? '';
    changed = null;
  });

  const main = $derived((r.providers.find((p) => p.id === r.provider) ?? { name: r.provider }).name);
  const picked = $derived(r.providers.find((p) => p.id === provider));
  const needsKey = $derived(!!picked && picked.key_required && !picked.has_key);
  const status = $derived<Mark | null>(
    changed ??
      (saved && r.fallback?.provider === saved.provider && r.fallback?.model === saved.model
        ? mark(true, saved.text, '')
        : r.fallback
          ? mark(r.fallback.ready, 'Only if ' + main + ' fails', 'No key: skipped')
          : null),
  );

  async function pick(e: Event & { currentTarget: HTMLSelectElement }) {
    provider = e.currentTarget.value;
    saved = null;
    if (!provider) {
      try {
        await commands.fallbackClear(r.role);
      } catch (e) {
        s.fail(e);
      }
      await s.refresh();
      return;
    }
    model = picked?.models[0]?.id ?? '';
    changed = { text: 'Not saved until Test passes', tone: 'warn' };
  }

  async function go() {
    const m = model.trim();
    if (!provider || !m) {
      s.fail('Pick a fallback provider and model.');
      return;
    }
    changed = testing;
    try {
      if (key.trim() && needsKey) {
        await commands.keySet(r.role, key, provider);
        key = '';
      }
      const ms = await commands.fallbackSet(r.role, provider, m);
      s.error = '';
      saved = { provider, model: m, text: `Works · ${ms} ms · saved` };
      await s.refresh();
    } catch (e) {
      changed = mark(false, '', String(e) + ' Not saved.');
    }
  }
</script>

<tr id="fallback-{r.role}">
  <td class="faint">Fallback</td>
  <td>
    <select aria-label="{r.label} fallback provider" value={provider} onchange={pick}>
      <option value="">None</option>
      {#each r.providers as p (p.id)}
        {#if p.id !== 'local'}<option value={p.id}>{p.name}</option>{/if}
      {/each}
    </select>
  </td>
  <td><input type="text" placeholder="Model" aria-label="{r.label} fallback model" bind:value={model} /></td>
  <td>
    <div class="key">
      <State mark={status} id="fallback-state-{r.role}" /><input
        type="password"
        placeholder="Paste the key"
        autocomplete="off"
        aria-label="{r.label} fallback key"
        class:hidden={!needsKey}
        bind:value={key}
      /><button onclick={go}>Test and save</button>
    </div>
  </td>
</tr>
