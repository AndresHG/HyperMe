<script lang="ts">
  // Settings → Connectors (specs/97-google-connectors, slice 5): the list, the Add sheet, the
  // connect dialog and an account's page. Draws what `connectors_view` returns; every rule lives
  // in Rust.
  import { onMount } from 'svelte';
  import { sheet } from '../../home/settings.svelte';
  import { commands } from '../../lib/ipc';
  import { useSettings } from '../state.svelte';
  import ApprovalCard from './ApprovalCard.svelte';
  import ConnectDialog from './ConnectDialog.svelte';
  import ConnectorPage from './ConnectorPage.svelte';
  import ConnectorRows from './ConnectorRows.svelte';

  let { on }: { on: boolean } = $props();
  const s = useSettings();
  const data = $derived(s.connectors);

  let mode = $state<'list' | 'sheet' | 'oauth'>('list');
  /** The connect dialog's preset, and the account it signs in again, if any. */
  let connecting = $state<{ connector: string; label: string; features: string[] | null } | null>(null);
  let custom = $state(false);
  const form = $state({ name: '', url: '', command: '', auth: 'none' });

  // `connectors/<id>` opens that account's page (hyperme://settings/connectors/<id>).
  const page = $derived(s.section === 'connectors' && s.item ? data?.pages.find((p) => p.id === s.item) : undefined);
  const view = $derived(mode === 'list' ? (page ? 'page' : 'list') : mode);

  $effect(() => {
    void sheet.visit;
    mode = 'list';
  });

  function connect(connector: string, label = '', features: string[] | null = null) {
    if (!data?.sheets.some((x) => x.connector === connector)) {
      s.fail('No setup sheet for ' + connector);
      return;
    }
    connecting = { connector, label, features };
    mode = 'oauth';
  }

  async function addCustom() {
    try {
      const id = await commands.connectorsAddCustom({ ...form });
      s.error = '';
      await s.refreshConnectors();
      s.go('connectors/' + id);
    } catch (e) {
      s.fail(e);
    }
  }

  onMount(() => {
    s.refreshConnectors().catch(s.fail);
  });
</script>

<section id="connectors" class:on>
  <ApprovalCard />
  <div class:hidden={view !== 'list'}>
    <div class="row" style="border:0;padding-top:0">
      <div>
        <h1>Connectors</h1>
        <p class="lead" style="margin:0">MCP servers Hyper reads from and acts in, each with its accounts and tools.</p>
      </div>
      <button class="main" style="white-space:nowrap" onclick={() => ((custom = false), (mode = 'sheet'))}>+ Add connector</button>
    </div>
    <div>
      <ConnectorRows rows={data?.rows ?? []} words="connector account" onpick={(r) => s.go('connectors/' + r.id)} />
    </div>
    <p class="lead" class:hidden={!data || data.rows.length > 0}>No connectors yet. Add one to let Hyper read your calendar, mail or tracker.</p>
  </div>
  <div class:hidden={view !== 'sheet'}>
    <h1>Add connector</h1>
    <p class="lead">A preset, or your own MCP server.</p>
    <div>
      {#each data?.choices ?? [] as c (c.id)}
        <div class="row" data-words="add connector {c.name}">
          <div class="what">{c.name}<small>{c.note}</small></div>
          <button disabled={!c.ready} onclick={() => (c.id === 'custom' ? (custom = true) : connect(c.id))}>Set up</button>
        </div>
      {/each}
    </div>
    <div class:hidden={!custom}>
      <div class="row" data-words="custom mcp server name">
        <div class="what">Name</div>
        <input type="text" placeholder="Team wiki" bind:value={form.name} />
      </div>
      <div class="row" data-words="custom mcp server url">
        <div class="what">URL<small>https://, or http:// to this Mac</small></div>
        <input type="text" placeholder="https://wiki.example.com/mcp" bind:value={form.url} />
      </div>
      <div class="row" data-words="custom mcp server command stdio">
        <div class="what">Or a command<small>Servers started by a command are not supported yet.</small></div>
        <input type="text" placeholder="my-mcp-server" disabled bind:value={form.command} />
      </div>
      <div class="row" data-words="custom mcp server bearer token sign in">
        <div class="what">Sign-in</div>
        <select bind:value={form.auth}><option value="none">None</option><option value="bearer">Bearer token</option></select>
      </div>
      <div class="row" style="border:0">
        <span></span><span><button onclick={() => (custom = false)}>Cancel</button> <button class="main" onclick={addCustom}>Add</button></span>
      </div>
    </div>
    <p><button onclick={() => (mode = 'list')}>Cancel</button></p>
  </div>
  {#if view === 'oauth' && connecting}
    <ConnectDialog {...connecting} onclose={() => s.go('connectors')} />
  {/if}
  {#if view === 'page' && page}
    <ConnectorPage {page} onsignin={() => connect(page.id.split(':')[0] ?? page.id, page.label, page.features)} />
  {/if}
</section>
