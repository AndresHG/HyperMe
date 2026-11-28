<script lang="ts">
  // The Dictionary page (spec 93, R-10 to R-13, R-15 to R-19), in Home only, in Wispr Flow's
  // layout (V0.1 redesign, owner, 27 November 2026). One add row, always open: type the word and
  // press Add or ↵; the "Correct a misspelling" switch turns it into misspelling → word, and Edit
  // loads a row into it. The whole list is saved on each change through `dictionary_save`.
  // Learning: a Suggested block on top while fixes wait, a "learned" mark with Undo in place of
  // Remove, and the "Learn from my edits" switch at the bottom.
  import { tick } from 'svelte';
  import { SvelteSet } from 'svelte/reactivity';
  import { commands, type DictionaryView } from '../lib/ipc';
  import { Switch } from '../lib/ui';
  import { key, toEntries, toRows, type Row } from './dictionary';
  import Icon from './Icon.svelte';

  let { say }: { say: (message: string) => void } = $props();

  let rows = $state<Row[]>([]);
  let view = $state<DictionaryView | null>(null);
  let learning = $state(true);
  let editing = $state(-1);
  let misspelling = $state(false);
  let wrong = $state('');
  let right = $state('');
  let hint = $state('');
  let submitLabel = $state('Add');
  let search = $state('');
  let typed = $state('');
  let confirmedCommon = '';
  // Suggestions warned about once as common words (R-17): the next Add goes ahead.
  const warned = new SvelteSet<string>();
  let wrongInput = $state<HTMLInputElement>();
  let rightInput = $state<HTMLInputElement>();
  let file = $state<HTMLInputElement>();

  const learned = $derived(view?.learned ?? []);
  const isLearned = (r: Row) => learned.some((l) => key(l.written) === key(r.word) && key(l.heard) === key(r.wrong));
  const q = $derived(key(search || typed));
  const shown = $derived(rows.map((r, i) => ({ r, i })).filter(({ r }) => !q || key(r.word + ' ' + r.wrong).includes(q)));

  async function focusField() {
    await tick();
    (misspelling ? wrongInput : rightInput)?.focus();
  }

  function open(i: number) {
    editing = i;
    confirmedCommon = '';
    const row = rows[i] ?? { word: '', wrong: '' };
    misspelling = !!row.wrong;
    wrong = row.wrong;
    right = row.word;
    hint = '';
    typed = '';
    submitLabel = i >= 0 ? 'Save' : 'Add';
    focusField();
  }
  const close = () => open(-1);

  function show(v: DictionaryView) {
    view = v;
    learning = v.learning !== false;
    rows = toRows(v.entries);
    if (v.error) say(v.error);
  }

  async function save(next: Row[]) {
    try {
      show(await commands.dictionarySave(toEntries(next)));
      say('');
      return true;
    } catch (e) {
      say(String(e));
      return false;
    }
  }

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    const word = right.trim();
    const heard = misspelling ? wrong.trim() : '';
    if (misspelling && !heard) return wrongInput?.focus();
    if (!word) return rightInput?.focus();
    if (heard.includes(',')) {
      hint = 'One misspelling at a time: add each as its own row.';
      return;
    }
    // R-17: a common misspelling would be replaced everywhere, so it needs a second press.
    if (heard && confirmedCommon !== key(heard) && (await commands.dictionaryIsCommon(heard))) {
      confirmedCommon = key(heard);
      hint = `“${heard}” is a common word. HyperMe will replace it everywhere.`;
      submitLabel = 'Add anyway';
      return;
    }
    const next = rows.slice();
    const row = { word, wrong: heard };
    if (editing >= 0) next[editing] = row;
    else next.unshift(row);
    if (await save(next)) close();
  }

  // R-18, R-19: undoing a learned row removes it, and it is never learned again.
  function remove(i: number, r: Row) {
    const mine = isLearned(r);
    // Undoing the last learned misspelling of a word the user added keeps the word.
    const last = !rows.some((o, j) => j !== i && key(o.word) === key(r.word));
    const wordLearned = learned.some((l) => key(l.written) === key(r.word) && !l.heard);
    save(
      mine && r.wrong && last && !wordLearned
        ? rows.map((o, j) => (j === i ? { word: r.word, wrong: '' } : o))
        : rows.filter((_, j) => j !== i),
    );
  }

  async function suggestion(s: DictionaryView['suggestions'][number], add: boolean) {
    const id = key(s.written) + '|' + key(s.heard);
    if (add && !warned.has(id) && (await commands.dictionaryIsCommon(s.heard))) {
      warned.add(id);
      say(`“${s.heard}” is a common word. HyperMe will replace it everywhere.`);
      return;
    }
    try {
      show(await commands.dictionarySuggestion(s.written, s.heard, add));
      say('');
    } catch (e) {
      say(String(e));
    }
  }

  async function setLearning(on: boolean) {
    try {
      learning = await commands.dictionaryLearning(on);
      say('');
    } catch (e) {
      say(String(e));
    }
  }

  async function importFile() {
    const chosen = file?.files?.[0];
    if (file) file.value = '';
    if (!chosen) return;
    try {
      show(await commands.dictionaryImport(chosen.name, await chosen.text()));
      say('');
    } catch (e) {
      say(String(e));
    }
  }

  async function exportAs(format: 'csv' | 'text') {
    try {
      await commands.dictionaryExport(format);
      say('');
    } catch (e) {
      say(String(e));
    }
  }

  commands
    .dictionaryView()
    .then(show)
    .catch((e) => say(String(e)));
</script>

<div id="dict-root" class="dict">
  {#if view?.suggestions.length}
    <div class="dict-suggested">
      {#each view.suggestions as s (s.written + '|' + s.heard)}
        <div class="dict-suggestion">
          <div class="dict-what">
            You fixed <span class="dict-wrong-word">{s.heard}</span> to <span class="dict-word">{s.written}</span>
            {s.seen > 1 ? s.seen + ' times' : 'once'}{s.app ? ' in ' + s.app : ''}. Add it?
          </div>
          <span class="dict-actions">
            <button class="main" onclick={() => suggestion(s, true)}>{warned.has(key(s.written) + '|' + key(s.heard)) ? 'Add anyway' : 'Add'}</button>
            <button onclick={() => suggestion(s, false)}>Not now</button>
          </span>
        </div>
      {/each}
    </div>
  {/if}

  <!-- Esc anywhere in the add row leaves an edit. -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <form class="dict-sheet" class:editing={editing >= 0} onsubmit={submit} onkeydown={(e) => e.key === 'Escape' && close()}>
    <div class="dict-fields">
      <input
        class="dict-wrong"
        class:hidden={!misspelling}
        bind:this={wrongInput}
        bind:value={wrong}
        type="text"
        maxlength="60"
        placeholder="Misspelling, as it comes out"
        aria-label="Misspelling"
        oninput={() => {
          if (confirmedCommon) {
            confirmedCommon = '';
            hint = '';
            submitLabel = editing >= 0 ? 'Save' : 'Add';
          }
        }}
      />
      <span class="dict-arrow" class:hidden={!misspelling}>→</span>
      <!-- The list narrows as the word is typed, so a word already there shows before it is added. -->
      <input
        class="dict-right"
        bind:this={rightInput}
        bind:value={right}
        type="text"
        maxlength="60"
        placeholder={misspelling ? 'Correct spelling' : 'Word, as it should be written'}
        aria-label={misspelling ? 'Correct spelling' : 'Word'}
        oninput={() => {
          if (editing < 0 && !misspelling) typed = right;
        }}
      />
      <button type="button" class:hidden={editing < 0} onclick={close}>Cancel</button>
      <button type="submit" class="main">{submitLabel}</button>
    </div>
    <label class="dict-switch"><Switch bind:checked={misspelling} onchange={focusField} /><span>Correct a misspelling</span></label>
    <p class="dict-hint">{hint}</p>
  </form>

  <input class="dict-search" bind:value={search} type="search" placeholder="Search your words" aria-label="Search your words" />

  {#if rows.length}
    <div class="dict-row dict-head"><span>Word</span><span>Often heard as</span><span></span></div>
  {/if}
  <div class="dict-rows">
    {#each shown as { r, i } (i)}
      {@const mine = isLearned(r)}
      <div class="dict-row">
        <div class="dict-what">
          <span class="dict-word">{r.word}</span>
          {#if mine}<span class="dict-learned">learned</span>{/if}
        </div>
        <span class="dict-heard">{r.wrong || '—'}</span>
        <span class="dict-actions">
          <button type="button" class="dict-icon" title="Edit" aria-label="Edit" onclick={() => open(i)}><Icon name="pencil" size={13} stroke={1.5} /></button>
          {#if mine}
            <button class="dict-undo" onclick={() => remove(i, r)}>Undo</button>
          {:else}
            <button type="button" class="dict-icon" title="Remove" aria-label="Remove" onclick={() => remove(i, r)}
              ><Icon name="trash" size={13} stroke={1.5} /></button
            >
          {/if}
        </span>
      </div>
    {/each}
  </div>
  {#if view && !rows.length}
    <p class="dict-empty">No words yet. Add names, products and jargon, or fix a word from a dictation on Home.</p>
  {/if}

  <!-- R-16: on by default; what it reads stays on the Mac (R-21). -->
  <label class="dict-switch dict-learn"><Switch bind:checked={learning} onchange={(e) => setLearning(e.currentTarget.checked)} /><span>Learn from my edits</span></label>
  <p class="dict-note">After a dictation, HyperMe notices when you fix a word. What it reads stays on your Mac.</p>

  <div class="dict-foot">
    <small class="dict-where">{view ? 'Saved on this Mac in ' + view.path : ''}</small>
    <span class="dict-tools">
      <button onclick={() => file?.click()}>Import…</button>
      <button class="dict-link" onclick={() => exportAs('csv')}>Export CSV</button>
      <button class="dict-link" onclick={() => exportAs('text')}>Export text</button>
    </span>
    <input bind:this={file} type="file" accept=".txt,.csv,text/plain,text/csv" hidden onchange={importFile} />
  </div>
</div>

<style>
  .dict .hidden { display: none !important; }
  .dict button { padding: var(--space-1) var(--space-3); border-radius: var(--radius-control); border: 1px solid var(--color-separator); background: transparent; color: inherit; font: inherit; cursor: default; }
  .dict button:hover { border-color: var(--color-accent); }
  .dict button.main { background: var(--color-accent); color: var(--color-on-accent); border-color: var(--color-accent); font-weight: 600; }
  .dict-sheet { border: 1px solid var(--color-separator); border-radius: var(--radius-sheet); padding: var(--space-3); display: flex; flex-direction: column; gap: var(--space-3); margin-top: var(--space-1); }
  .dict-sheet.editing { border-color: var(--color-accent); }
  .dict-switch { display: inline-flex; align-items: center; gap: var(--space-2); width: fit-content; color: var(--color-label-secondary); }
  .dict-fields { display: flex; align-items: center; gap: var(--space-2); }
  .dict-fields input { flex: 1; min-width: 0; padding: var(--space-2) var(--space-3); border-radius: var(--radius-control); border: 1px solid var(--color-separator); background: transparent; color: inherit; font: inherit; outline: none; }
  .dict-fields input:focus { border-color: var(--color-accent); }
  .dict-arrow { color: var(--color-label-secondary); }
  .dict-hint { margin: 0; color: var(--color-orange-text); font-size: var(--text-callout); }
  .dict-hint:empty { display: none; }
  .dict-search { width: 100%; margin: var(--space-4) 0 var(--space-2); padding: var(--space-2) var(--space-3); border-radius: var(--radius-control); border: 1px solid var(--color-separator); background: transparent; color: inherit; font: inherit; outline: none; }
  .dict-search:focus { border-color: var(--color-accent); }
  .dict-row { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr) 72px; gap: var(--space-3); align-items: center; min-height: 42px; padding: 0 var(--space-3); border-bottom: 1px solid var(--color-separator); }
  .dict-row:hover { background: var(--color-hover); }
  .dict-head, .dict-head:hover { min-height: 30px; background: none; font-size: var(--text-subheadline); font-weight: 600; letter-spacing: .06em; text-transform: uppercase; color: var(--color-label-secondary); }
  .dict-what { display: flex; align-items: center; gap: var(--space-2); min-width: 0; }
  .dict-word { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .dict-heard { color: var(--color-label-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .dict-wrong-word { color: var(--color-label-secondary); text-decoration: line-through; }
  .dict-actions { display: flex; gap: var(--space-1); justify-content: flex-end; visibility: hidden; }
  .dict-row:hover .dict-actions, .dict-row:focus-within .dict-actions { visibility: visible; }
  .dict .dict-icon { width: 28px; height: 28px; padding: 0; display: flex; align-items: center; justify-content: center; }
  .dict .dict-undo { padding: var(--space-1) var(--space-2); font-size: var(--text-callout); }
  .dict-empty { color: var(--color-label-secondary); padding: var(--space-3) var(--space-3); }
  .dict-suggested { display: flex; flex-direction: column; gap: var(--space-2); margin-bottom: var(--space-3); }
  .dict-suggestion { display: flex; align-items: center; gap: var(--space-3); padding: var(--space-3) var(--space-4); border-radius: var(--radius-sheet); background: var(--color-accent-soft); }
  .dict-suggestion .dict-what { flex: 1; flex-wrap: wrap; gap: var(--space-1); }
  .dict-suggestion .dict-actions { visibility: visible; }
  .dict-learned { color: var(--color-accent-text); background: var(--color-accent-soft); font-size: var(--text-subheadline); border-radius: var(--radius-capsule); padding: 0 var(--space-2); font-weight: 500; }
  .dict-learn { margin-top: var(--space-5); }
  .dict-note { margin: var(--space-1) 0 0 var(--space-8); color: var(--color-label-secondary); font-size: var(--text-callout); }
  .dict-foot { display: flex; justify-content: space-between; align-items: center; gap: var(--space-3); margin-top: var(--space-5); color: var(--color-label-secondary); }
  .dict-where { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .dict-tools { display: flex; gap: var(--space-2); flex-shrink: 0; }
  .dict .dict-link { border: 0; color: var(--color-label-secondary); padding: var(--space-1) var(--space-2); }
  .dict .dict-link:hover { color: inherit; }
</style>
