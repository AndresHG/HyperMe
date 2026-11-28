<script lang="ts">
  // A connector account's page: on or off, its sign-in, the approvals its servers wait on
  // (specs/166), today's requests, and each tool's access.
  import { commands, type ConnectorPage, type ToolAccess } from '../../lib/ipc';
  import { tone, useSettings } from '../state.svelte';

  let { page, onsignin }: { page: ConnectorPage; onsignin: () => void } = $props();
  const s = useSettings();
  const connector = $derived(page.id.split(':')[0] ?? page.id);
  let token = $state('');
  const reviewed = s.reviewed;
  let checking = $state<string | null>(null);

  const WORDS = 'approval needs your ok not allowed blocked held back review ask again';
  const ACCESS: [ToolAccess, string][] = [
    ['read', 'Read'],
    ['ask', 'Ask before'],
    ['off', 'Off'],
  ];
  // A Review that cleared its state leaves no row: its result shows on its own.
  const cleared = $derived(
    [...reviewed].filter(([key]) => {
      const [account, server] = key.split(' ');
      return account === page.id && !page.approvals.some((l) => l.server === server);
    }),
  );

  const refresh = () => s.refreshConnectors().catch(s.fail);

  async function review(server: string) {
    const key = page.id + ' ' + server;
    checking = key;
    try {
      reviewed.set(key, await commands.connectorsReview(page.id, server));
    } catch (e) {
      reviewed.set(key, String(e));
    }
    checking = null;
    await refresh();
  }

  async function saveToken() {
    if (!token.trim()) return;
    try {
      await commands.connectorsSetToken(page.id, token);
      token = '';
      await refresh();
    } catch (e) {
      s.fail(e);
    }
  }

  async function disconnect() {
    if (!confirm(`Disconnect ${page.title}? This removes access and deletes what HyperMe cached from this account.`)) return;
    try {
      await commands.connectorsDisconnect(page.id);
      s.go('connectors');
      await refresh();
    } catch (e) {
      s.fail(e);
      await refresh();
    }
  }
</script>

<div>
  <p><a href="#connectors" onclick={(e) => (e.preventDefault(), s.go('connectors'))}>‹ Connectors</a></p>
  <h1>{page.title}</h1>
  <p class="lead"><span class="state {tone(page.tone)}">{page.state}</span></p>
  <div class="row" class:hidden={!page.account} data-words="connector account on off">
    <div class="what">Use this account<small>Off keeps the sign-in but stops using it.</small></div>
    <input
      type="checkbox"
      checked={page.enabled}
      onchange={(e) => commands.connectorsSetEnabled(page.id, e.currentTarget.checked).then(refresh).catch(s.fail)}
    />
  </div>
  <div class="row" class:hidden={page.account || page.state !== 'Not signed in'} data-words="connector bearer token">
    <div class="what">Bearer token<small>Kept in the macOS keychain, never shown again.</small></div>
    <span><input type="password" placeholder="Paste a token" autocomplete="off" bind:value={token} /> <button onclick={saveToken}>Save</button></span>
  </div>
  <div class="row" class:hidden={!(page.account && page.oauth)} data-words="sign in again disconnect account">
    <div class="what">Account<small>Disconnect removes access and deletes what HyperMe cached from this account.</small></div>
    <span style="white-space:nowrap"><button onclick={onsignin}>Sign in again</button> <button onclick={disconnect}>Disconnect</button></span>
  </div>
  <div>
    {#each page.approvals as line, i (i)}
      {#if !line.action}
        <div class="row" data-words={WORDS}><div class="what"><small>{line.text}</small></div></div>
      {:else}
        {@const key = page.id + ' ' + line.server}
        <div class="row" data-words={WORDS}>
          <div class="what">{line.text}<small>{checking === key ? 'Checking…' : (reviewed.get(key) ?? '')}</small></div>
          <button disabled={checking === key} onclick={() => review(line.server)}>{line.action}</button>
        </div>
      {/if}
    {/each}
    {#each cleared as [key, text] (key)}
      <div class="row" data-words="review result"><div class="what"><small>{text}</small></div></div>
    {/each}
  </div>
  <div>
    {#each page.requests as line, i (i)}
      <div class="row" data-words="requests quota today"><div class="what"><small>{line}</small></div></div>
    {/each}
  </div>
  <p class="lead" style="margin-top:var(--space-4)">
    Tools<span class="warn" class:hidden={page.tools_verified}>&#32;· names not checked against the live server yet</span>
  </p>
  <div>
    {#each page.tools as t (t.name)}
      <div class="row" data-words="tool {t.name}">
        <div class="what"><code>{t.name}</code></div>
        <select
          aria-label={t.name}
          value={t.access}
          onchange={(e) => commands.connectorsSetTool(connector, t.name, e.currentTarget.value as ToolAccess).then(refresh).catch(s.fail)}
        >
          {#each ACCESS as [value, title] (value)}<option {value}>{title}{value === t.default ? ' (default)' : ''}</option>{/each}
        </select>
      </div>
    {:else}
      <p class="lead">Tools appear after the first connection.</p>
    {/each}
  </div>
</div>
