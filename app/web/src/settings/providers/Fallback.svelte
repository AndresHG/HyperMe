<script lang="ts">
  // A role's fallback (specs/96, R-24 to R-24a): one line, "If it fails, try …", and Change to pick
  // another. It is saved only once its Test passes; a provider with no key yet takes it here (one
  // key per provider, D-2).
  import { commands, type RoleView } from '../../lib/ipc';
  import StateMark from '../StateMark.svelte';
  import { mark, testing, useSettings, type Mark } from '../state.svelte';

  let { r }: { r: RoleView } = $props();
  const s = useSettings();

  let editing = $state(false);
  let provider = $state('');
  let model = $state('');
  let key = $state('');
  let status = $state<Mark | null>(null);
  // A redraw from Rust closes the editor, as it did in the table.
  $effect.pre(() => {
    provider = r.fallback?.provider ?? '';
    model = r.fallback?.model ?? '';
    editing = false;
  });

  const main = $derived((r.providers.find((p) => p.id === r.provider) ?? { name: r.provider }).name);
  const picked = $derived(r.providers.find((p) => p.id === provider));
  const needsKey = $derived(!!picked && picked.key_required && !picked.has_key);
  const line = $derived<Mark | null>(
    status ?? (r.fallback ? mark(r.fallback.ready, 'Only if ' + main + ' fails', 'No key: skipped until you add it') : null),
  );

  function edit() {
    editing = true;
    status = null;
  }

  async function clear() {
    try {
      await commands.fallbackClear(r.role);
      status = null;
    } catch (e) {
      s.fail(e);
    }
    await s.refresh();
  }

  function pick(e: Event & { currentTarget: HTMLSelectElement }) {
    provider = e.currentTarget.value;
    model = picked?.models[0]?.id ?? '';
    status = { text: 'Not saved until Test passes', tone: 'warn' };
  }

  async function save() {
    const m = model.trim();
    if (!provider || !m) {
      s.fail('Pick a fallback provider and model.');
      return;
    }
    status = testing;
    try {
      if (key.trim() && needsKey) {
        await commands.keySet(r.role, key, provider);
        key = '';
      }
      const ms = await commands.fallbackSet(r.role, provider, m);
      s.error = '';
      status = mark(true, `Works · ${ms} ms · saved`, '');
      await s.refresh();
    } catch (e) {
      status = mark(false, '', String(e) + ' Not saved.');
    }
  }
</script>

<div class="fallback" id="fallback-{r.role}" data-search data-words="fallback backup if it fails">
  <div class="line">
    <span>
      {#if r.fallback}If it fails, try <b>{r.fallback.name} · {r.fallback.model}</b>{:else}No fallback set.{/if}
    </span>
    {#if !editing}
      <span>
        <a class="link" href="#fallback" onclick={(e) => (e.preventDefault(), edit())}>{r.fallback ? 'Change' : 'Add'}</a>
        {#if r.fallback}· <a class="link" href="#fallback" onclick={(e) => (e.preventDefault(), clear())}>Remove</a>{/if}
      </span>
    {/if}
  </div>
  <StateMark small mark={line} id="fallback-state-{r.role}" />
  {#if editing}
    <div class="edit">
      <select aria-label="{r.label} fallback provider" value={provider} onchange={pick}>
        <option value="" disabled>Provider</option>
        {#each r.providers as p (p.id)}
          {#if p.id !== 'local' && p.id !== r.provider}<option value={p.id}>{p.name}</option>{/if}
        {/each}
      </select>
      <input type="text" placeholder="Model" aria-label="{r.label} fallback model" bind:value={model} />
      {#if needsKey}<input type="password" placeholder="Paste the key" autocomplete="off" aria-label="{r.label} fallback key" bind:value={key} />{/if}
      <button class="main" onclick={save}>Test and save</button>
      <button onclick={() => (editing = false)}>Cancel</button>
    </div>
  {/if}
</div>

<style>
  .fallback { border-top: var(--border-width) solid var(--color-separator); padding-top: var(--space-3); margin-top: var(--space-4); color: var(--color-label-secondary); font-size: var(--text-callout); }
  .line { display: flex; align-items: center; justify-content: space-between; gap: var(--space-3); }
  b { color: var(--color-label); font-weight: var(--weight-medium); }
  .fallback :global(small.state) { display: block; margin-top: var(--space-1); }
  .edit { display: flex; gap: var(--space-2); margin-top: var(--space-2); }
  .edit input, .edit select { flex: 1; min-width: 0; }
</style>
