<script lang="ts">
  // One role in the providers table (specs/96-provider-settings): its provider, model, key and Test
  // (R-6 to R-14), the hint line under it (R-18), a local server or custom endpoint being added
  // (R-21, R-22), and its fallback (R-24, R-24a).
  import { commands, type ProviderHint, type ProviderView, type RoleView } from '../../lib/ipc';
  import StateMark from '../StateMark.svelte';
  import { mark, testing, useSettings, type Mark } from '../state.svelte';
  import FallbackRow from './FallbackRow.svelte';
  import LocalServer from './LocalServer.svelte';

  let { r }: { r: RoleView } = $props();
  const s = useSettings();

  const pending = $derived(s.pending[r.role]);
  const want = $derived(pending ?? { provider: r.provider, model: r.model });
  const chosen = $derived<ProviderView>(
    r.providers.find((p) => p.id === want.provider) ?? {
      id: want.provider,
      name: want.provider,
      models: [],
      key_required: true,
      has_key: false,
      has_key_page: false,
      key_note: null,
      recommended: false,
    },
  );
  const inUse = $derived(r.providers.find((p) => p.id === r.provider));
  const needsKey = $derived(chosen.key_required && !chosen.has_key);

  /** The menu while a custom endpoint or local server is being added. */
  let picked = $state<string | null>(null);
  let model = $state('');
  let key = $state('');
  // A redraw from Rust starts the row over, as the table did when it was drawn again.
  let tested = $state<{ for: RoleView; mark: Mark } | null>(null);
  let live = $state<string[] | null>(null);
  let notice = $state('');
  $effect.pre(() => {
    model = want.model;
    picked = null;
    live = null;
    notice = '';
  });

  const keyMark = $derived<Mark>(
    tested?.for === r
      ? tested.mark
      : needsKey
        ? { text: pending ? `No key yet. Until this one works, dictation keeps using ${inUse ? inUse.name : r.provider}.` : 'No key yet', tone: 'warn' }
        : { text: chosen.key_required ? 'Key saved' : 'No key needed', tone: 'ok' },
  );

  // R-15: the key's own models, fetched when the field is first focused and cached by the app.
  async function fetchModels() {
    if (live) return;
    try {
      const menu = await commands.modelsList(chosen.id);
      live = menu.models;
      notice = menu.notice ?? '';
    } catch (e) {
      notice = String(e);
    }
  }
  const suggestions = $derived(live ?? chosen.models.map((m) => m.id));

  const efforts = $derived.by(() => {
    const info = chosen.models.find((m) => m.id === want.model);
    const own = info ? info.reasoning_effort : r.reasoning_effort && r.reasoning_effort !== 'off' ? [r.reasoning_effort] : [];
    return ['off', ...own];
  });

  // The hint line: cost, measured speed with its source, and the user's own median (R-18).
  let hint = $state<ProviderHint | null>(null);
  $effect(() => {
    hint = null;
    if (!want.model) return;
    commands
      .providerHint(r.role, chosen.id, want.model)
      .then((h) => (hint = h))
      .catch(() => (hint = null));
  });

  async function choose(provider: string, model: string) {
    try {
      const result = await commands.providerChoose(r.role, provider, model);
      s.error = '';
      if (result === 'needs_key') s.pending[r.role] = { provider, model };
      else delete s.pending[r.role];
    } catch (e) {
      s.fail(e);
    }
    await s.refresh();
  }

  function pick(e: Event & { currentTarget: HTMLSelectElement }) {
    const id = e.currentTarget.value;
    if (id === '__add' || id === 'local') {
      picked = id;
      return;
    }
    const p = r.providers.find((x) => x.id === id);
    if (!p) return;
    const first = p.models[0]?.id ?? '';
    if (!first) s.pending[r.role] = { provider: p.id, model: '' };
    else choose(p.id, first);
  }

  async function test(): Promise<boolean> {
    const at = r;
    tested = { for: at, mark: testing };
    try {
      const ms = await commands.keyTest(r.role, chosen.id, model.trim());
      tested = { for: at, mark: mark(true, `Works · ${ms} ms`, '') };
      return true;
    } catch (e) {
      tested = { for: at, mark: mark(false, '', String(e)) };
      return false;
    }
  }

  // R-12: the key goes to the keychain, the pair is tested, and only then does the role switch.
  async function saveAndTest() {
    if (!key.trim()) return;
    const m = model.trim();
    if (!m) {
      s.fail('Type a model name first.');
      return;
    }
    try {
      await commands.keySet(r.role, key, chosen.id);
      key = '';
    } catch (e) {
      s.fail(e);
      return;
    }
    if (await test()) await choose(chosen.id, m);
  }

  // A custom endpoint for cleanup (R-21).
  let addName = $state('');
  let addAddress = $state('');
  async function add() {
    try {
      const id = await commands.providerAdd(addName, addAddress);
      s.error = '';
      s.pending[r.role] = { provider: id, model: '' };
      await s.refresh();
    } catch (e) {
      s.fail(e);
    }
  }

  function focusOnMount(node: HTMLElement) {
    node.focus();
  }
</script>

<tr id="role-{r.role}">
  <td>{r.label}</td>
  <td>
    <select aria-label="{r.label} provider" value={picked ?? chosen.id} disabled={picked !== null} onchange={pick}>
      {#each r.providers as p (p.id)}
        <option value={p.id}>{p.name}{p.recommended ? ' (Recommended)' : ''}</option>
      {/each}
      {#if r.role === 'cleanup'}<option value="__add">Add a custom endpoint…</option>{/if}
    </select>
    {#if picked === '__add'}
      <div class="add">
        <input type="text" placeholder="Name, such as My server" bind:value={addName} use:focusOnMount />
        <input type="text" placeholder="https://host/v1" bind:value={addAddress} />
        <button class="main" onclick={add}>Add</button>
        <button onclick={() => (picked = null)}>Cancel</button>
      </div>
    {/if}
  </td>
  <td>
    <input
      type="text"
      placeholder="Type a model name"
      aria-label="{r.label} model"
      list="models-{r.role}"
      bind:value={model}
      onfocus={fetchModels}
      onchange={() => model.trim() && choose(chosen.id, model.trim())}
    />
    <datalist id="models-{r.role}">
      {#each suggestions as m (m)}<option value={m}></option>{/each}
    </datalist>
    <div class="effort">{notice}</div>
    {#if r.role === 'cleanup' && !pending}
      <div class="effort">
        Reasoning effort<select
          aria-label="Reasoning effort"
          value={r.reasoning_effort}
          onchange={(e) => s.save('cleanup', 'reasoning_effort', e.currentTarget.value).then(() => s.refresh())}
        >
          {#each efforts as e (e)}<option value={e}>{e}</option>{/each}
        </select>
      </div>
    {/if}
  </td>
  <td>
    <div class="key">
      <StateMark mark={keyMark} id="role-state-{r.role}" />
      {#if needsKey}
        <input type="password" placeholder="Paste the key" autocomplete="off" bind:value={key} />
        <button class="main" onclick={saveAndTest}>Save and test</button>
        {#if chosen.has_key_page}
          <a class="link" href="#key" onclick={(e) => (e.preventDefault(), commands.providerKeyPage(chosen.id))}>Get a key</a>
        {/if}
      {:else}
        <button onclick={test}>Test</button>
      {/if}
    </div>
    {#if needsKey && chosen.key_note}<div class="faint key-note">{chosen.key_note}</div>{/if}
  </td>
</tr>
{#if picked === 'local'}
  <LocalServer oncancel={() => (picked = null)} />
{/if}
<tr>
  <td class="hint" colspan="4" id="hint-{r.role}">
    {#if hint}{hint.cost} · {hint.speed}{#if hint.source}<a class="link" href="#results" onclick={(e) => (e.preventDefault(), hint?.source && commands.resultOpen(hint.source))}
          >&#32;(results)</a
        >{/if}{hint.mine ? ' · ' + hint.mine : ''}{/if}
  </td>
</tr>
<FallbackRow {r} />
