<script lang="ts">
  // Settings → AI models: one page per role (owner, 28 November 2026; docs/ux/settings-models/).
  // Where it runs (cloud, or a server on this Mac for cleanup), the provider as tabs, its key
  // masked with Edit, its models with the one in use marked Active, any other model by name with a
  // Test, the hint line, and the fallback. The rules are specs/96-provider-settings's (R-6 to R-28).
  import { commands, type ProviderHint, type ProviderView, type Role } from '../../lib/ipc';
  import StateMark from '../StateMark.svelte';
  import { mark, testing, useSettings, type Mark } from '../state.svelte';
  import Fallback from './Fallback.svelte';
  import LocalServer from './LocalServer.svelte';

  let { role, id, title, sub, on }: { role: Role; id: string; title: string; sub: string; on: boolean } = $props();
  const s = useSettings();
  const r = $derived(s.view?.roles.find((x) => x.role === role));

  // The provider shown: one picked without a key waits in `pending` until its key passes (R-12).
  const pending = $derived(s.pending[role]);
  const shownId = $derived(pending?.provider ?? r?.provider ?? '');
  const shown = $derived<ProviderView | undefined>(r?.providers.find((p) => p.id === shownId));
  const inUse = $derived(r?.providers.find((p) => p.id === r.provider));
  const cloud = $derived(r?.providers.filter((p) => p.id !== 'local') ?? []);
  const local = $derived(r?.providers.find((p) => p.id === 'local'));
  const onMac = $derived(shownId === 'local');
  const needsKey = $derived(!!shown && shown.key_required && !shown.has_key);

  let mode = $state<'cloud' | 'mac' | null>(null);
  let adding = $state(false);
  let editingKey = $state(false);
  let key = $state('');
  let keyMark = $state<Mark | null>(null);
  let other = $state('');
  let otherMark = $state<Mark | null>(null);
  let otherWorks = $state(false);
  let showAll = $state(false);
  // A new provider starts its key, model and test lines over.
  $effect.pre(() => {
    void shownId;
    mode = null;
    adding = false;
    editingKey = false;
    key = '';
    keyMark = null;
    other = '';
    otherMark = null;
    otherWorks = false;
    showAll = false;
  });
  const where = $derived(mode ?? (onMac ? 'mac' : 'cloud'));

  // The model list: the catalogue's own first, then what the key lists where the provider can
  // list its models (R-15), fetched when the page shows that provider.
  let live = $state<{ provider: string; models: string[]; notice: string } | null>(null);
  $effect(() => {
    const provider = shownId;
    if (!on || !provider || onMac || live?.provider === provider) return;
    commands
      .modelsList(provider)
      .then((menu) => (live = { provider, models: menu.models, notice: menu.notice ?? '' }))
      .catch((e) => (live = { provider, models: [], notice: String(e) }));
  });
  const SHORT = 8;
  const models = $derived.by(() => {
    const own = shown?.models.map((m) => m.id) ?? [];
    const listed = live?.provider === shownId ? live.models : [];
    const all = [...own, ...listed.filter((m) => !own.includes(m))];
    // The model in use is always in the list, even when typed by hand.
    const current = pending?.model || (shownId === r?.provider ? r?.model : '');
    if (current && !all.includes(current)) all.unshift(current);
    return all;
  });
  const visible = $derived(showAll ? models : models.slice(0, SHORT));
  const activeModel = $derived(shownId === r?.provider && !pending ? r?.model : null);

  // The hint line for the model in use: cost, measured speed, and the user's own median (R-18).
  let hint = $state<ProviderHint | null>(null);
  $effect(() => {
    hint = null;
    const model = activeModel;
    if (!model || !r) return;
    commands
      .providerHint(role, r.provider, model)
      .then((h) => (hint = h))
      .catch(() => (hint = null));
  });

  const efforts = $derived.by(() => {
    const info = shown?.models.find((m) => m.id === activeModel);
    const own = info ? info.reasoning_effort : r?.reasoning_effort && r.reasoning_effort !== 'off' ? [r.reasoning_effort] : [];
    return ['off', ...own];
  });

  async function choose(provider: string, model: string) {
    try {
      const result = await commands.providerChoose(role, provider, model);
      s.error = '';
      if (result === 'needs_key') s.pending[role] = { provider, model };
      else delete s.pending[role];
    } catch (e) {
      s.fail(e);
    }
    await s.refresh();
  }

  function pickProvider(p: ProviderView) {
    if (p.id === shownId) return;
    const first = p.models[0]?.id ?? '';
    if (!first || (p.key_required && !p.has_key)) s.pending[role] = { provider: p.id, model: first };
    else choose(p.id, first);
  }

  async function test(model: string): Promise<number | null> {
    try {
      return await commands.keyTest(role, shownId, model);
    } catch (e) {
      keyMark = mark(false, '', String(e));
      return null;
    }
  }

  // R-12: the key goes to the keychain, the pair is tested, and only then does the role switch.
  async function saveKey() {
    if (!key.trim() || !shown) return;
    const model = pending?.model || activeModel || shown.models[0]?.id || '';
    if (!model) {
      s.fail('Pick a model first.');
      return;
    }
    try {
      await commands.keySet(role, key, shown.id);
      key = '';
    } catch (e) {
      s.fail(e);
      return;
    }
    keyMark = testing;
    const ms = await test(model);
    if (ms === null) return;
    keyMark = mark(true, `Works · answered in ${ms} ms`, '');
    editingKey = false;
    if (pending || shown.id !== r?.provider) await choose(shown.id, model);
    else await s.refresh();
  }

  async function testKey() {
    keyMark = testing;
    const ms = await test(activeModel ?? pending?.model ?? '');
    if (ms !== null) keyMark = mark(true, `Works · answered in ${ms} ms`, '');
  }

  function pickModel(model: string) {
    if (model === activeModel) return;
    if (needsKey) s.pending[role] = { provider: shownId, model };
    else choose(shownId, model);
  }

  // "Other model…": any name the provider takes, tested before it is used.
  async function testOther() {
    const model = other.trim();
    if (!model) return;
    otherWorks = false;
    otherMark = testing;
    try {
      const ms = await commands.keyTest(role, shownId, model);
      otherMark = mark(true, `Works · answered in ${ms} ms`, '');
      otherWorks = true;
    } catch (e) {
      otherMark = mark(false, '', String(e));
    }
  }

  // A custom endpoint (R-21): any OpenAI-compatible server, here or on the network.
  let addName = $state('');
  let addAddress = $state('');
  async function add() {
    try {
      const provider = await commands.providerAdd(addName, addAddress);
      s.error = '';
      s.pending[role] = { provider, model: '' };
      await s.refresh();
    } catch (e) {
      s.fail(e);
    }
  }

  async function reset() {
    try {
      await commands.providersReset();
      s.pending = {};
      s.error = '';
    } catch (e) {
      s.fail(e);
    }
    await s.refresh();
  }
</script>

{#snippet modeRow(value: 'cloud' | 'mac', name: string, note: string, active: boolean)}
  <label class="mode" class:on={where === value}>
    <div class="t"><b>{name}</b>{#if active}<span class="badge">Active</span>{/if}<div>{note}</div></div>
    <input type="radio" name="{id}-mode" class="ui-checkbox" checked={where === value} onchange={() => (mode = value)} />
  </label>
{/snippet}

<section {id} class:on>
  <h1>{title}</h1>
  <p class="lead">{sub}</p>
  {#if r}
    {#if local}
      <div class="modes" data-search data-words="cloud on this mac local ollama lm studio private server">
        {@render modeRow('cloud', 'Cloud providers', 'Your own API key.', !onMac)}
        {@render modeRow('mac', 'On this Mac', 'A model server such as Ollama or LM Studio. Fully private.', onMac)}
      </div>
    {/if}

    {#if where === 'mac'}
      {#if onMac}
        <p class="note">Using <b>{r.model}</b> on this Mac. Nothing leaves it.</p>
        <div class="label">Model</div>
      {/if}
      <LocalServer oncancel={() => (mode = 'cloud')} />
    {:else}
      <div class="providers" role="tablist" aria-label="{title} provider" data-search data-words="provider {cloud.map((p) => p.name).join(' ')} custom endpoint">
        {#each cloud as p (p.id)}
          <button class="prov" class:on={p.id === shownId} role="tab" aria-selected={p.id === shownId} onclick={() => pickProvider(p)}>
            <span class="mono">{p.name.slice(0, 1)}</span>{p.name}{p.recommended ? ' (Recommended)' : ''}
          </button>
        {/each}
        {#if role === 'cleanup'}
          <button class="prov" class:on={adding} onclick={() => (adding = !adding)}><span class="mono">+</span>Custom</button>
        {/if}
      </div>
      {#if adding}
        <div class="add" data-search data-words="custom endpoint server openai compatible">
          <input type="text" placeholder="Name, such as My server" aria-label="Server name" bind:value={addName} />
          <input type="text" placeholder="https://host/v1" aria-label="Server address" bind:value={addAddress} />
          <button class="main" onclick={add}>Add</button>
          <button onclick={() => (adding = false)}>Cancel</button>
        </div>
      {/if}

      {#if shown}
        {#if pending}
          <p class="state warn">Until {shown.name} works, dictation keeps using {inUse ? inUse.name : r.provider}.</p>
        {/if}
        <div data-search data-words="api key test keychain {shown.name}">
          <div class="label">
            API key
            {#if shown.has_key_page}
              <a class="link" href="#key" onclick={(e) => (e.preventDefault(), commands.providerKeyPage(shown.id))}>Get a key →</a>
            {/if}
          </div>
          {#if !shown.key_required}
            <p class="note">No key needed.</p>
          {:else if shown.has_key && !editingKey}
            <!-- The key itself never comes back from the keychain, only that one is saved. -->
            <div class="key">
              <span class="dots">••••••••••••••••</span>
              <button class="ui-button borderless small" onclick={testKey}>Test</button>
              <button class="ui-button borderless small" onclick={() => (editingKey = true)}>Edit</button>
            </div>
          {:else}
            <div class="add">
              <input type="password" placeholder="Paste the key" autocomplete="off" aria-label="{shown.name} key" bind:value={key} />
              <button class="main" onclick={saveKey}>Save and test</button>
              {#if editingKey}<button onclick={() => (editingKey = false)}>Cancel</button>{/if}
            </div>
            {#if shown.key_note}<p class="note">{shown.key_note}</p>{/if}
          {/if}
          <StateMark small mark={keyMark} id="role-state-{role}" />
        </div>

        <div class="label">Model</div>
        <div class="models" role="radiogroup" aria-label="{title} model" data-search data-words="model {models.join(' ')}">
          {#each visible as m (m)}
            {@const active = m === activeModel}
            {@const picked = m === pending?.model}
            <button class="model" class:on={active || picked} role="radio" aria-checked={active} onclick={() => pickModel(m)}>
              <i></i><b>{m}</b>
              {#if active}<span class="badge">Active</span>{:else if picked}<span class="badge">After the key works</span>{/if}
            </button>
          {/each}
          {#if models.length > SHORT}
            <button class="more" onclick={() => (showAll = !showAll)}>{showAll ? 'Show fewer' : `Show all ${models.length} models`}</button>
          {/if}
        </div>
        {#if live?.provider === shownId && live.notice}<p class="note">{live.notice}</p>{/if}
        <div class="other" data-search data-words="other model name custom test">
          <input type="text" placeholder="Other model…" aria-label="Other model name" bind:value={other} oninput={() => ((otherWorks = false), (otherMark = null))} />
          <button onclick={testOther} disabled={!other.trim()}>Test</button>
          {#if otherWorks}<button class="main" onclick={() => pickModel(other.trim())}>Use it</button>{/if}
        </div>
        <StateMark small mark={otherMark} />
        {#if hint}
          <p class="note hint" id="hint-{role}">
            {hint.cost} · {hint.speed}{#if hint.source}<a
                class="link"
                href="#results"
                onclick={(e) => (e.preventDefault(), hint?.source && commands.resultOpen(hint.source))}>&#32;(results)</a
              >{/if}{hint.mine ? ' · ' + hint.mine : ''}
          </p>
        {/if}
        {#if role === 'cleanup' && !pending}
          <div class="effort" data-search data-words="reasoning effort thinking">
            Reasoning effort
            <select aria-label="Reasoning effort" value={r.reasoning_effort} onchange={(e) => s.save('cleanup', 'reasoning_effort', e.currentTarget.value).then(() => s.refresh())}>
              {#each efforts as e (e)}<option value={e}>{e}</option>{/each}
            </select>
          </div>
        {/if}
      {/if}
    {/if}

    <Fallback {r} />
    <p class="foot"><a class="link" href="#reset" onclick={(e) => (e.preventDefault(), reset())}>Reset to recommended</a></p>
  {/if}
</section>

<style>
  .modes { border: var(--border-width) solid var(--color-separator); border-radius: var(--radius-card); overflow: hidden; margin-bottom: var(--space-4); background: var(--color-card-bg); }
  .mode { display: flex; align-items: center; gap: var(--space-3); padding: var(--space-3) var(--space-4); }
  .mode + .mode { border-top: var(--border-width) solid var(--color-separator); }
  .mode .t { flex: 1; }
  .mode .t b { font-weight: var(--weight-medium); }
  .mode .t div { color: var(--color-label-secondary); font-size: var(--text-callout); }
  .mode input { accent-color: var(--color-accent); margin: 0; }
  .badge {
    display: inline-block; margin-left: var(--space-2); padding: 0 var(--space-2); border-radius: var(--radius-small); background: var(--color-accent-soft);
    color: var(--color-accent-text); font-size: var(--text-caption); font-weight: var(--weight-medium); white-space: nowrap;
  }
  .providers { display: flex; gap: var(--space-2); flex-wrap: wrap; margin-bottom: var(--space-4); }
  .prov {
    display: inline-flex; align-items: center; gap: var(--space-2); height: var(--control-height-large); padding: 0 var(--space-3) 0 var(--space-1);
    border-radius: var(--radius-capsule); border: var(--border-width) solid var(--color-separator); background: transparent; font-size: var(--text-callout);
  }
  .prov.on { background: var(--color-selected); border-color: transparent; font-weight: var(--weight-medium); }
  .mono {
    width: 22px; height: 22px; border-radius: var(--radius-capsule); display: grid; place-items: center; background: var(--color-fill);
    color: var(--color-label-secondary); font-size: var(--text-caption); font-weight: var(--weight-bold);
  }
  .prov.on .mono { background: var(--color-accent); color: var(--color-on-accent); }
  .label { display: flex; justify-content: space-between; align-items: baseline; font-weight: var(--weight-medium); margin: var(--space-3) 0 var(--space-2); }
  .label .link { font-weight: var(--weight-regular); font-size: var(--text-callout); }
  .key {
    display: flex; align-items: center; gap: var(--space-1); height: var(--control-height-large); padding: 0 var(--space-1) 0 var(--field-padding-x);
    border-radius: var(--radius-control); border: var(--border-width) solid var(--color-control-border); background: var(--color-control-bg);
  }
  .key .dots { flex: 1; font-family: var(--font-mono); color: var(--color-label-secondary); }
  .key button { color: var(--color-accent-text); }
  .add, .other { display: flex; gap: var(--space-2); margin-bottom: var(--space-1); }
  .add input, .other input { flex: 1; min-width: 0; }
  .models { display: grid; gap: var(--space-1); margin-bottom: var(--space-2); }
  .model {
    display: flex; align-items: center; justify-content: flex-start; gap: var(--space-2); height: 38px; padding: 0 var(--space-3);
    border-radius: var(--radius-control); border: var(--border-width) solid var(--color-separator); background: var(--color-card-bg); text-align: left;
  }
  .model i { width: 6px; height: 6px; border-radius: var(--radius-capsule); background: var(--color-label-quaternary); flex: none; }
  .model.on { border-color: var(--color-accent); background: var(--color-accent-soft); }
  .model.on i { background: var(--color-accent); }
  .model b { flex: 1; font-weight: var(--weight-medium); overflow: hidden; text-overflow: ellipsis; }
  .more { justify-self: start; border: 0; background: transparent; color: var(--color-accent-text); padding: 0 var(--space-1); }
  .note { margin: 0 0 var(--space-2); color: var(--color-label-secondary); font-size: var(--text-callout); }
  .note b { color: var(--color-label); font-weight: var(--weight-medium); }
  .hint { margin-top: var(--space-2); }
  .effort { display: flex; align-items: center; gap: var(--space-2); margin-top: var(--space-2); color: var(--color-label-secondary); font-size: var(--text-callout); }
  .foot { margin: var(--space-4) 0 0; font-size: var(--text-callout); }
  section :global(small.state) { display: block; margin: var(--space-1) 0 var(--space-2); }
  section :global(small.state:empty) { display: none; }
</style>
