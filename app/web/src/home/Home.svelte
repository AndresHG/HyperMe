<script lang="ts">
  // The Home window (docs/UX.md, "Home"; R-5, R-6): the sidebar and one section at a time, chosen
  // by the address's hash (`#history`, `#home/brief`). Settings opens as a sheet over it.
  import { onMount } from 'svelte';
  import { commands, on, type HomeInfo, type Section } from '../lib/ipc';
  import { Sheet } from '../lib/ui';
  import Dictionary from './Dictionary.svelte';
  import History from './History.svelte';
  import { stateOf, say } from './rows.svelte';
  import { closeSettings, openSettings, sheet } from './settings.svelte';
  import Sidebar from './Sidebar.svelte';
  import TodayPage from './TodayPage.svelte';

  let sections = $state<Section[]>([]);
  let info = $state<HomeInfo>({ history: true, dictionary: false });
  let attention = $state(false);
  let ready = $state(false);
  let hash = $state(location.hash);
  // A new value redraws the section from scratch (`home://refresh`).
  let generation = $state(0);
  let dictError = $state('');
  let scroller = $state<HTMLElement>();

  const route = $derived.by(() => {
    const [section = '', block] = hash.replace(/^#/, '').split('/');
    const id = sections.some((s) => s.id === section) ? section : 'home';
    return { id, block: block ?? null };
  });

  // Rust reaches the sheet through these.
  const w = window as Window & { openSettings?: (s: string) => void; closeSettings?: () => void };
  w.openSettings = openSettings;
  w.closeSettings = closeSettings;

  function onkeydown(e: KeyboardEvent) {
    // The sheet keeps the keyboard while it is open, as its frame once did.
    if (!e.metaKey || e.altKey || e.ctrlKey || sheet.open) return;
    // ⌘, opens Settings at the section closest to where you are (docs/UX.md).
    if (e.key === ',') {
      e.preventDefault();
      openSettings(route.id === 'history' ? 'privacy' : 'general');
      return;
    }
    const n = Number(e.key);
    const target = sections[n - 1];
    if (n >= 1 && n <= 5 && target) {
      e.preventDefault();
      location.hash = target.id;
    }
  }

  // ---- Events from the app (R-16, R-22) ----
  on('home://attention', (on) => (attention = on));
  on('home://refresh', () => generation++);
  // A retry's progress on its row, wherever the row is (R-16).
  on('home://retry', ({ id, state, message }) => {
    const row = stateOf(id);
    if (state === 'working') {
      row.working = true;
      return;
    }
    row.working = false;
    if (state === 'done') {
      row.trace = undefined;
      say(id, 'Retried · new text copied');
    } else if (state === 'failed') {
      say(id, message ?? 'Failed', true);
    }
  });

  onMount(async () => {
    [info, sections] = await Promise.all([commands.homeInfo(), commands.homeSections()]);
    attention = await commands.homeAttention(null);
    ready = true;
    const pending = await commands.settingsPending();
    if (pending) openSettings(pending);
  });
</script>

<svelte:window {onkeydown} onhashchange={() => (hash = location.hash)} />

<div id="titlebar" data-tauri-drag-region></div>
<Sidebar {sections} current={route.id} {attention} />
<main id="main" bind:this={scroller}>
  {#if ready && scroller}
    {#key `${route.id}/${route.block}/${generation}`}
      {#if route.id === 'history'}
        <History {info} {scroller} />
      {:else if route.id === 'dictionary'}
        <!-- Spec 93, D4: the Dictionary page lives only here, as in Wispr Flow (V0.1 redesign). -->
        <section id="dictionary" aria-label="Dictionary">
          <header>
            <div>
              <h1>Dictionary</h1>
              <p class="lead">Words HyperMe should always get right. Turn on "Correct a misspelling" to fix a word that keeps coming out wrong.</p>
            </div>
          </header>
          <p class="dict-error" id="dict-error" role="alert">{dictError}</p>
          <Dictionary say={(m) => (dictError = m)} />
        </section>
      {:else}
        <TodayPage block={route.block} />
      {/if}
    {/key}
  {/if}
</main>
<Sheet bind:open={() => sheet.open, (open) => !open && closeSettings()} label="Settings">
  <!-- Its own chunk, fetched the first time Settings opens (R-10). -->
  {#await import('../settings/Settings.svelte') then { default: Settings }}
    <Settings />
  {/await}
</Sheet>

<style>
  .dict-error { color: var(--color-red); margin: var(--space-2) 0 0; }
  .dict-error:empty { display: none; }
</style>
