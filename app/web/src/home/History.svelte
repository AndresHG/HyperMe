<script lang="ts">
  // History (R-10 to R-19): every dictation, newest first, in pages as you scroll; search with
  // the matches marked; the keys and the ⌘K menu for the selected row. While history is off it
  // lists only the dictations still waiting for a retry.
  import { onDestroy, tick } from 'svelte';
  import { commands, on, type HomeInfo, type Item } from '../lib/ipc';
  import { Menu, type MenuItem } from '../lib/ui';
  import { queryWords } from './format';
  import HistoryList from './HistoryList.svelte';
  import Icon from './Icon.svelte';
  import { forget, say, stateOf } from './rows.svelte';
  import { openSettings } from './settings.svelte';

  const PAGE = 50;

  let { info, scroller }: { info: HomeInfo; scroller: HTMLElement } = $props();

  let items = $state<Item[]>([]);
  let query = $state('');
  let words = $state<string[]>([]);
  let more = $state('');
  let selected = $state<number | null>(null);
  let list = $state<HistoryList>();
  let search = $state<HTMLInputElement>();
  let menu = $state({ open: false, x: 0, y: 0 });
  let before: [number, number] | null = null;
  let done = false;
  let loading = false;
  let token = 0;

  const itemOf = (id: number) => items.find((i) => i.id === id);
  const has = (id: number, action: string) => (itemOf(id)?.actions ?? []).some((a) => a.id === action);
  const selectedItem = $derived(selected === null ? undefined : itemOf(selected));
  const menuItems = $derived<MenuItem[]>((selectedItem?.actions ?? []).map((a) => ({ id: a.id, label: a.label, key: a.key })));

  function reset() {
    before = null;
    done = false;
    token += 1;
    selected = null;
    words = queryWords(query);
    items = [];
    more = '';
    loadMore();
  }

  async function loadMore() {
    if (loading || done) return;
    loading = true;
    const mine = token;
    try {
      const page = !info.history
        ? await commands.homeFailed()
        : query.trim()
          ? await commands.homeSearch(query, before, PAGE)
          : await commands.homePage(before, PAGE);
      if (mine !== token) return;
      items = [...items, ...page];
      const last = page[page.length - 1];
      if (last) before = [last.started_ms, last.id];
      done = !info.history || page.length < PAGE;
      if (items.length === 0) {
        more = !info.history ? 'No dictation is waiting for a retry.' : query.trim() ? 'Nothing matches.' : 'No dictations yet.';
      } else {
        more = done ? '' : 'Scroll for more';
      }
    } catch (error) {
      if (mine === token) more = String(error);
    } finally {
      loading = false;
    }
  }

  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  function searched() {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(reset, 120);
  }

  // ---- Selection and actions (R-13 to R-18) ----
  async function select(id: number | null, scroll = true) {
    if (selected === id) return;
    menu.open = false;
    if (selected !== null) stateOf(selected).mode = null;
    selected = id;
    if (id === null) return;
    if (scroll) list?.reveal(id);
    await tick();
    list?.row(id)?.focus();
  }

  function move(step: number) {
    if (items.length === 0) return;
    const at = selected === null ? -1 : items.findIndex((i) => i.id === selected);
    const next = at === -1 ? (step > 0 ? 0 : items.length - 1) : Math.max(0, Math.min(items.length - 1, at + step));
    const item = items[next];
    if (item) select(item.id);
    if (step > 0 && next >= items.length - 5) loadMore();
  }

  function openMenu() {
    if (selected === null || !selectedItem) return;
    const box = list?.row(selected)?.box() ?? { right: window.innerWidth - 40, top: 100 };
    const n = selectedItem.actions.length;
    menu = { open: true, x: Math.max(12, box.right - 220), y: Math.min(window.innerHeight - 40 * n - 12, box.top + 30) };
  }

  async function run(id: number, action: string) {
    if (!has(id, action)) return;
    const state = stateOf(id);
    try {
      if (action === 'copy') {
        await commands.homeCopy(id);
        say(id, 'Copied');
      }
      if (action === 'paste') {
        const outcome = await commands.homePaste(id);
        say(id, outcome.pasted ? 'Pasted' : outcome.notice);
      }
      if (action === 'retry') {
        await commands.homeRetry(id);
        state.working = true;
      }
      if (action === 'fix') {
        state.mode = { kind: 'fix-select' };
        offerFix(id);
      }
      if (action === 'correct') state.mode = { kind: 'correct', text: itemOf(id)?.text ?? '' };
      if (action === 'delete-sample') {
        await commands.homeDeleteSample(id);
        const item = await commands.homeItem(id);
        if (item) replace(item);
        say(id, 'Sample deleted');
      }
      if (action === 'delete') state.mode = { kind: 'confirm-delete' };
    } catch (error) {
      say(id, String(error), true);
    }
  }

  async function confirmDelete(id: number) {
    try {
      await commands.homeDelete(id);
      const at = items.findIndex((i) => i.id === id);
      const next = items[at + 1] ?? items[at - 1] ?? null;
      selected = null;
      forget(id);
      items = items.filter((i) => i.id !== id);
      if (next) select(next.id);
    } catch (error) {
      stateOf(id).mode = null;
      say(id, String(error), true);
    }
  }

  function replace(item: Item) {
    items = items.map((i) => (i.id === item.id ? item : i));
  }

  // Fix a word (R-17): a selection inside the selected row's text starts it, once asked for.
  function offerFix(id: number) {
    const item = itemOf(id);
    const state = stateOf(id);
    if (!info.dictionary || !item || !has(id, 'fix')) return;
    if (!state.mode || (state.mode.kind !== 'fix-select' && state.mode.kind !== 'fix-type')) return;
    const chosen = String(window.getSelection() || '').trim();
    if (!chosen || !(item.text || '').includes(chosen)) return;
    state.mode = { kind: 'fix-type', heard: chosen, wanted: '' };
  }

  async function sendFix(id: number, confirmed: boolean) {
    const state = stateOf(id);
    const mode = state.mode;
    if (!mode || (mode.kind !== 'fix-type' && mode.kind !== 'fix-confirm')) return;
    try {
      const outcome = await commands.homeFixWord(id, mode.heard, mode.wanted, confirmed);
      if (outcome.outcome === 'needs_confirm') {
        state.mode = { kind: 'fix-confirm', heard: mode.heard, wanted: mode.wanted };
        return;
      }
      state.mode = null;
      const item = await commands.homeItem(id);
      if (item) replace(item);
      say(id, 'Fixed. Press ↵ to paste it again');
    } catch (error) {
      state.mode = null;
      say(id, String(error), true);
    }
  }

  // Correct (spec 267, R-4 to R-7): the whole text as it should have been, kept with the audio as
  // a sample for benchmarks. The first save explains what is kept and waits for a yes (R-6).
  async function sendCorrect(id: number, agreed: boolean) {
    const state = stateOf(id);
    const mode = state.mode;
    if (!mode || (mode.kind !== 'correct' && mode.kind !== 'correct-agree')) return;
    try {
      const outcome = await commands.homeCorrect(id, mode.text, agreed);
      if (outcome.outcome === 'needs_agreement') {
        state.mode = { kind: 'correct-agree', text: mode.text };
        return;
      }
      state.mode = null;
      const item = await commands.homeItem(id);
      if (item) replace(item);
      say(id, outcome.changed ? 'Saved as a sample. Press ↵ to paste it again' : 'Saved as a sample');
    } catch (error) {
      say(id, String(error), true);
    }
  }

  // ---- Keys (R-13) ----
  function onkeydown(e: KeyboardEvent) {
    keys(e);
    typeToSearch(e);
  }

  function keys(e: KeyboardEvent) {
    const typing = e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement;
    const inSearch = e.target === search;
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      if (typing && !inSearch) return;
      e.preventDefault();
      if (typing) (e.target as HTMLInputElement).blur();
      move(e.key === 'ArrowDown' ? 1 : -1);
      return;
    }
    if (selected === null || (typing && !inSearch)) return;
    const id = selected;
    const mode = stateOf(id).mode;
    if (e.key === 'Escape') {
      e.preventDefault();
      if (mode) stateOf(id).mode = null;
      else select(null);
      return;
    }
    if (e.key === 'Enter' && !typing) {
      e.preventDefault();
      if (mode?.kind === 'confirm-delete') confirmDelete(id);
      else if (mode?.kind === 'fix-confirm') sendFix(id, true);
      else if (mode?.kind === 'correct-agree') sendCorrect(id, true);
      else if (!mode) run(id, 'paste');
      return;
    }
    if (!e.metaKey || e.altKey || e.ctrlKey || typing) return;
    if (e.key === 'c' && String(window.getSelection() || '') === '') {
      e.preventDefault();
      run(id, 'copy');
    } else if (e.key === 'r') {
      e.preventDefault();
      run(id, 'retry');
    } else if (e.key === 'f') {
      e.preventDefault();
      run(id, 'fix');
    } else if (e.key === 'e') {
      e.preventDefault();
      run(id, 'correct');
    } else if (e.key === 'Backspace') {
      e.preventDefault();
      run(id, 'delete');
    } else if (e.key === 'k') {
      e.preventDefault();
      openMenu();
    }
  }

  // Typing anywhere in History goes to the search field. It is not focused on entry: a focused
  // field kept about 20 MB of graphics memory alive (AC-2).
  function typeToSearch(e: KeyboardEvent) {
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement || e.key.length !== 1 || !info.history) return;
    if (selected !== null && stateOf(selected).mode) return;
    search?.focus();
  }

  // Pages load as the end of the list comes near.
  $effect(() => {
    const onScroll = () => {
      if (scroller.scrollTop + scroller.clientHeight > scroller.scrollHeight - 600) loadMore();
    };
    scroller.addEventListener('scroll', onScroll, { passive: true });
    return () => scroller.removeEventListener('scroll', onScroll);
  });

  // A dictation made while Home is open goes on top of the list (AC-15).
  async function showNew() {
    try {
      const page = info.history ? await commands.homePage(null, PAGE) : await commands.homeFailed();
      if (query.trim() || items.length === 0) return;
      const newest = items[0]?.started_ms ?? -Infinity;
      const fresh = page.filter((i) => !itemOf(i.id) && i.started_ms >= newest);
      if (fresh.length) items = [...fresh, ...items];
    } catch {
      // The next open shows them.
    }
  }

  // A retry that ends brings the item's new text (R-16); its row state follows in Home.
  const stops = [
    on('home://changed', () => showNew()),
    on('home://retry', async ({ id, state }) => {
      if (state !== 'done') return;
      const item = await commands.homeItem(id);
      if (item) replace(item);
    }),
  ];
  onDestroy(() => {
    clearTimeout(searchTimer);
    stops.forEach((s) => s.then((off) => off()));
  });

  reset();
</script>

<svelte:window {onkeydown} />

<section id="history" aria-labelledby="history-title">
  <header>
    <div>
      <h1 id="history-title">History</h1>
      <p class="lead">Everything you dictated, newest first.</p>
    </div>
    {#if info.history}
      <label class="search" id="search-box">
        <Icon name="search" size={14} stroke={1.5} />
        <input
          id="search"
          bind:this={search}
          bind:value={query}
          oninput={searched}
          type="search"
          placeholder="Search dictations"
          aria-label="Search dictations"
          autocomplete="off"
        />
      </label>
    {/if}
  </header>
  {#if !info.history}
    <div class="off" id="off">
      History is off · <a href="#privacy" id="off-link" onclick={(e) => (e.preventDefault(), openSettings('privacy'))}>Settings → Privacy</a>. Only
      dictations waiting for a retry are listed.
    </div>
  {/if}
  <HistoryList
    bind:this={list}
    {items}
    {words}
    {selected}
    {scroller}
    onselect={(id) => select(id, false)}
    onrun={run}
    onmenu={openMenu}
    onfixtext={offerFix}
    onsendfix={(id) => sendFix(id, false)}
    onsendcorrect={(id) => sendCorrect(id, false)}
  />
  <div class="more" id="more">{more}</div>
</section>

<Menu
  bind:open={menu.open}
  x={menu.x}
  y={menu.y}
  items={menuItems}
  onpick={(action) => {
    if (selected !== null) run(selected, action);
  }}
/>

<style>
  .search { display: flex; align-items: center; gap: var(--space-2); width: 240px; flex: none; padding: var(--space-2) var(--space-3); border: 1px solid var(--color-separator); border-radius: var(--radius-control); background: var(--color-content-bg); color: var(--color-label-secondary); }
  .search:focus-within { border-color: var(--color-accent); box-shadow: 0 0 0 3px color-mix(in srgb, var(--color-accent) 22%, transparent); }
  .search input { border: 0; outline: none; background: transparent; color: var(--color-label); font: inherit; flex: 1; min-width: 0; }
  .more { color: var(--color-label-secondary); font-size: var(--text-callout); padding: var(--space-3) 0; text-align: center; }
  .off { padding: var(--space-3) var(--space-3); border: 1px solid var(--color-separator); border-radius: var(--radius-sheet); background: var(--color-card-bg); margin-bottom: var(--space-3); color: var(--color-label-secondary); }
  .off a { color: var(--color-accent-text); text-decoration: none; }
</style>
