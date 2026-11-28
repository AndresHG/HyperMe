<script lang="ts">
  // Settings, a sheet inside Home (V0.1 redesign; spec 257, R-10): the sidebar of sections and
  // one section at a time. Its code is its own chunk, loaded the first time the sheet opens; it is
  // made fresh each time the sheet opens and dropped when it closes.
  import { onMount, tick } from 'svelte';
  import { sheet } from '../home/settings.svelte';
  import { commands } from '../lib/ipc';
  import About from './About.svelte';
  import Advanced from './Advanced.svelte';
  import Connectors from './connectors/Connectors.svelte';
  import Diagnostics from './Diagnostics.svelte';
  import General from './General.svelte';
  import Privacy from './Privacy.svelte';
  import Providers from './providers/Providers.svelte';
  import Setup from './Setup.svelte';
  import Shortcuts from './Shortcuts.svelte';
  import { provideSettings, Settings } from './state.svelte';
  import './settings.css';

  const s = provideSettings(new Settings());

  let root = $state<HTMLElement>();
  let main = $state<HTMLElement>();
  let query = $state('');
  /** While searching: the sections with a matching row. */
  let found = $state<string[]>([]);

  const links: ({ id: string; title: string } | string)[] = [
    { id: 'setup', title: 'Setup check' },
    'Everyday',
    { id: 'general', title: 'General' },
    { id: 'shortcuts', title: 'Shortcuts' },
    'Setup',
    { id: 'providers', title: 'Providers' },
    { id: 'connectors', title: 'Connectors' },
    { id: 'privacy', title: 'Privacy' },
    { id: 'diagnostics', title: 'Diagnostics' },
    { id: 'advanced', title: 'Advanced' },
    { id: 'about', title: 'About' },
  ];

  /** Whether a section shows: the one chosen, or while searching, each with a matching row. */
  const shown = (id: string) => (query.trim() ? found.includes(id) : s.section === id);

  // Each visit opens its section at the top. From the walkthrough's "Use other providers"
  // (U-12): Providers, with Advanced showing.
  $effect(() => {
    void sheet.visit;
    if (sheet.place === 'providers-advanced') s.advanced = true;
    requestAnimationFrame(() => main && (main.scrollTop = 0));
  });

  // docs/UX.md rule 6: search finds every setting by its name and common words.
  $effect(() => {
    const q = query.trim().toLowerCase();
    tick().then(() => {
      if (!root) return;
      const sections: string[] = [];
      for (const row of root.querySelectorAll<HTMLElement>('main .row')) {
        const miss = !!q && !(row.textContent + ' ' + (row.dataset.words ?? '')).toLowerCase().includes(q);
        row.classList.toggle('miss', miss);
        if (!miss && !row.classList.contains('hidden')) sections.push(row.closest('section')?.id ?? '');
      }
      found = sections;
    });
  });

  // The sheet closes with its × or Esc (Home's Sheet), unless Esc belongs to a field with text.
  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && e.target instanceof HTMLInputElement && e.target.value) e.preventDefault();
  }

  onMount(() => {
    root?.focus();
    s.refresh();
  });
</script>

<div class="settings ui-native" bind:this={root} tabindex="-1" role="presentation" {onkeydown}>
  <nav>
    <input type="search" placeholder="Search settings" aria-label="Search settings" bind:value={query} />
    {#each links as link (typeof link === 'string' ? link : link.id)}
      {#if typeof link === 'string'}
        <div class="group">{link}</div>
      {:else if link.id !== 'advanced' || s.advanced}
        <a href={'#' + link.id} class:on={s.section === link.id} onclick={(e) => (e.preventDefault(), s.go(link.id))}>{link.title}</a>
      {/if}
    {/each}
  </nav>
  <main bind:this={main}>
    <button id="sheet-x" aria-label="Close settings" title="Close (Esc)" onclick={() => s.close()}>×</button>
    {#if s.banner}
      <div id="banner">
        <span>Saved. Restart HyperMe to use the new settings.</span>
        <button class="main" onclick={() => commands.appRestart()}>Restart HyperMe</button>
      </div>
    {/if}
    <Setup on={shown('setup')} />
    <General on={shown('general')} />
    <Shortcuts on={shown('shortcuts')} />
    <Providers on={shown('providers')} />
    <Connectors on={shown('connectors')} />
    <Privacy on={shown('privacy')} />
    <Diagnostics on={shown('diagnostics')} />
    <Advanced on={shown('advanced')} />
    <About on={shown('about')} />
    <p id="error" class="bad">{s.error}</p>
  </main>
</div>

<style>
  #sheet-x {
    position: fixed; top: 12px; right: 14px; width: 28px; height: 28px; padding: 0; border-radius: var(--radius-capsule);
    border: 0; background: var(--color-separator); font-size: var(--text-title3); line-height: 1; z-index: 5;
  }
  #sheet-x:hover { background: var(--color-control-border); }
  #banner {
    display: flex; position: sticky; top: 0; margin: calc(-1 * var(--space-6)) calc(-1 * var(--space-8)) var(--space-4);
    padding: var(--space-3) var(--space-8); background: var(--color-accent-soft); border-bottom: 1px solid var(--color-separator);
    align-items: center; justify-content: space-between;
  }
</style>
