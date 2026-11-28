<script lang="ts">
  // Settings → Providers (specs/96-provider-settings): the two keys, or with "Show advanced" the
  // table of roles (R-6 to R-14), each with its provider, model, key and fallback.
  import { sheet } from '../../home/settings.svelte';
  import { commands, type Role } from '../../lib/ipc';
  import StateMark from '../StateMark.svelte';
  import { mark, testing, useSettings, type Mark } from '../state.svelte';
  import RoleRows from './RoleRows.svelte';

  let { on }: { on: boolean } = $props();
  const s = useSettings();
  const v = $derived(s.view);
  const keys = $state<Record<Role, string>>({ speech: '', cleanup: '' });
  const tested = $state<Partial<Record<Role, Mark>>>({});

  // hyperme://settings/providers/speech, from the "keeps failing" notice (R-28): the table, with
  // that role's row in view.
  $effect(() => {
    void sheet.visit;
    const role = s.section === 'providers' ? s.item : null;
    if (!role) return;
    s.advanced = true;
    setTimeout(() => document.getElementById('role-' + role)?.scrollIntoView({ block: 'center' }), 0);
  });

  async function saveKey(role: Role) {
    if (!keys[role].trim()) return;
    try {
      await commands.keySet(role, keys[role]);
      keys[role] = '';
      delete tested[role];
      s.refresh();
    } catch (e) {
      s.fail(e);
    }
  }

  async function test(role: Role) {
    tested[role] = testing;
    try {
      tested[role] = mark(true, 'Works, ' + (await commands.keyTest(role)) + ' ms', '');
    } catch (e) {
      tested[role] = mark(false, '', String(e));
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

{#snippet simple(role: Role, title: string, use: string)}
  <div class="row" data-words="{role} provider {role === 'speech' ? 'speechmatics' : 'groq'} key test">
    <div class="what">
      {title}<small>{v?.config ? `${role === 'speech' ? v.config.speech_provider : v.config.cleanup_provider} · used by ${use}` : `Used by ${use}`}</small><StateMark
        small
        mark={tested[role] ?? (v && mark(v.keys[role], 'Key saved', 'No key yet'))}
      />
    </div>
    <span
      ><input type="password" placeholder="Paste a new key" autocomplete="off" bind:value={keys[role]} />
      <button onclick={() => saveKey(role)}>Save</button> <button onclick={() => test(role)}>Test</button></span
    >
  </div>
{/snippet}

<section id="providers" class:on>
  <h1>Providers</h1>
  <p class="lead">Every model service in one place. Keys stay in the macOS keychain and are never shown again.</p>
  <div class:hidden={s.advanced}>
    {@render simple('speech', 'Speech', 'dictation')}
    {@render simple('cleanup', 'Cleanup', 'dictation cleanup')}
    <p class="lead" style="margin-top:var(--space-3)">
      Turn on <a class="link" href="#general" onclick={(e) => (e.preventDefault(), s.go('general'))}>Show advanced</a> to choose other providers and models.
    </p>
  </div>
  <div class:hidden={!s.advanced} data-words="provider model openai openrouter gemini anthropic local custom endpoint reasoning">
    <table class="roles">
      <thead><tr><th style="width:15%">Role</th><th style="width:25%">Provider</th><th style="width:27%">Model</th><th>Key / Test</th></tr></thead>
      <tbody>
        {#each v?.roles ?? [] as r (r.role)}
          <RoleRows {r} />
        {/each}
      </tbody>
    </table>
    <div class="toolbar">
      <small class="lead" style="margin:0">Changes save at once and reach the next dictation.</small><button onclick={reset}>Reset to recommended</button>
    </div>
  </div>
</section>

<style>
  /* The table's cells are drawn by RoleRows, one role at a time. */
  .roles { width: 100%; border-collapse: collapse; margin-top: var(--space-2); }
  .roles th {
    text-align: left; font-weight: 500; color: var(--color-label-secondary); font-size: var(--text-callout);
    padding: var(--space-2) var(--space-2) var(--space-2) 0; border-bottom: 1px solid var(--color-separator);
  }
  .roles :global(td) { padding: var(--space-3) var(--space-2) var(--space-1) 0; vertical-align: top; }
  .roles :global(td.hint) {
    padding: 0 0 var(--space-3); color: var(--color-label-secondary); font-size: var(--text-callout); border-bottom: 1px solid var(--color-separator);
  }
  .roles :global(td.local) { padding-top: 0; }
  .roles :global(:is(select, input[type=text])) { width: 100%; }
  .roles :global(input[type=text]) { min-width: 0; }
  .roles :global(.key) { display: flex; flex-wrap: wrap; gap: var(--space-2); align-items: center; }
  .roles :global(.key input[type=password]) { min-width: 140px; flex: 1; }
  .roles :global(.effort) { margin-top: var(--space-2); font-size: var(--text-callout); color: var(--color-label-secondary); }
  .roles :global(.effort select) { width: auto; margin-left: var(--space-1); }
  .roles :global(.add) { display: flex; gap: var(--space-2); margin-top: var(--space-2); }
  .roles :global(.add :is(input, select)) { flex: 1; }
  .toolbar { display: flex; justify-content: space-between; align-items: center; margin-top: var(--space-5); }
</style>
