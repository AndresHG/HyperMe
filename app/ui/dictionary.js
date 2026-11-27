/* The Dictionary page (spec 93, R-10 to R-13, R-15, R-17), in Home only (V0.1 redesign, owner,
   27 November 2026; it was also in Settings). It works like Wispr Flow's: one field for a word; the
   "Correct a misspelling" switch turns it into two fields, the misspelling and the right word.
   Each misspelling is its own row ("zen trix → Zentrix"); a word with no misspelling is a row of
   its own. The whole list is saved on each change through `dictionary_save`.

   HyperDictionary.mount(root, { invoke, onError }) draws the page inside `root`. The add row is
   always open: there is no separate "Add word" step.

   Learning (spec 93, D6, R-16 to R-19): a Suggested block on top while fixes wait (Add,
   Dismiss), a "learned" mark on rows HyperMe learned with Undo in place of Remove, and the
   "Learn from my edits" switch at the bottom. */
(() => {
  const el = (tag, cls, text) => {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
  };
  // The matcher compares lower case without accents (ñ stays), so the page groups rows the same.
  const key = (s) => s.trim().toLowerCase().replace(/[áàéèíìóòúùü]/g, (c) => 'aaeeiioouuu'['áàéèíìóòúùü'.indexOf(c)]);

  // Entries ({written, heard[]}) to rows ({word, wrong}) and back.
  const toRows = (entries) => entries.flatMap((e) => (e.heard.length ? e.heard.map((h) => ({ word: e.written, wrong: h })) : [{ word: e.written, wrong: '' }]));
  function toEntries(rows) {
    const entries = [];
    for (const r of rows) {
      let entry = entries.find((e) => key(e.written) === key(r.word));
      if (!entry) { entry = { written: r.word, heard: [] }; entries.push(entry); }
      if (r.wrong && !entry.heard.some((h) => key(h) === key(r.wrong))) entry.heard.push(r.wrong);
    }
    return entries;
  }

  const PENCIL = '<path d="M10.5 2.5l3 3-8 8H2.5v-3z"/>';
  const TRASH = '<path d="M2.5 4.5h11M6 4.5V3h4v1.5M4 4.5l.7 9h6.6l.7-9"/>';
  function iconButton(label, paths) {
    const b = el('button', 'dict-icon');
    b.type = 'button';
    b.title = label;
    b.setAttribute('aria-label', label);
    b.innerHTML = '<svg aria-hidden="true" width="13" height="13" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">' + paths + '</svg>';
    return b;
  }

  function mount(root, { invoke, onError }) {
    const say = onError || (() => {});
    let rows = [];
    let editing = -1;
    let confirmedCommon = '';

    // One add row, always open (owner, 27 November 2026): type the word and press Add or ↵.
    // The switch turns it into misspelling → word. Edit loads a row into it; Cancel leaves.
    const sheet = el('form', 'dict-sheet');
    const fields = el('div', 'dict-fields');
    const wrong = el('input', 'dict-wrong');
    wrong.type = 'text';
    wrong.maxLength = 60;
    wrong.placeholder = 'Misspelling, as it comes out';
    wrong.setAttribute('aria-label', 'Misspelling');
    const arrow = el('span', 'dict-arrow', '→');
    const right = el('input', 'dict-right');
    right.type = 'text';
    right.maxLength = 60;
    const cancel = el('button', 'hidden', 'Cancel');
    cancel.type = 'button';
    const submit = el('button', 'main', 'Add');
    submit.type = 'submit';
    fields.append(wrong, arrow, right, cancel, submit);
    const switchLabel = el('label', 'dict-switch');
    const misspelling = el('input', 'ui-switch');
    misspelling.type = 'checkbox';
    misspelling.setAttribute('role', 'switch');
    switchLabel.append(misspelling, el('span', '', 'Correct a misspelling'));
    const hint = el('p', 'dict-hint');
    sheet.append(fields, switchLabel, hint);

    const search = el('input', 'dict-search');
    search.type = 'search';
    search.placeholder = 'Search your words';
    search.setAttribute('aria-label', 'Search your words');

    // R-18: fixes HyperMe noticed in the user's edits, waiting for a yes.
    const suggested = el('div', 'dict-suggested hidden');

    // A list in Wispr Flow's layout: the word, what it is often heard as, and its tools on hover.
    const head = el('div', 'dict-row dict-head');
    head.append(el('span', '', 'Word'), el('span', '', 'Often heard as'), el('span'));
    const list = el('div', 'dict-rows');
    const empty = el('p', 'dict-empty hidden', 'No words yet. Add names, products and jargon, or fix a word from a dictation on Home.');

    const foot = el('div', 'dict-foot');
    const where = el('small', 'dict-where');
    const tools = el('span', 'dict-tools');
    const importButton = el('button', '', 'Import…');
    const exportCsv = el('button', 'dict-link', 'Export CSV');
    const exportText = el('button', 'dict-link', 'Export text');
    tools.append(importButton, exportCsv, exportText);
    const file = el('input');
    file.type = 'file';
    file.accept = '.txt,.csv,text/plain,text/csv';
    file.hidden = true;
    foot.append(where, tools, file);

    // R-16: on by default; what it reads stays on the Mac (R-21).
    const learnLabel = el('label', 'dict-switch dict-learn');
    const learnSwitch = el('input', 'ui-switch');
    learnSwitch.type = 'checkbox';
    learnSwitch.setAttribute('role', 'switch');
    learnLabel.append(learnSwitch, el('span', '', 'Learn from my edits'));
    const learnNote = el('p', 'dict-note', 'After a dictation, HyperMe notices when you fix a word. What it reads stays on your Mac.');
    learnSwitch.addEventListener('change', async () => {
      try { learnSwitch.checked = await invoke('dictionary_learning', { on: learnSwitch.checked }); say(''); } catch (e) { say(String(e)); }
    });

    root.classList.add('dict');
    root.replaceChildren(suggested, sheet, search, head, list, empty, learnLabel, learnNote, foot);
    let learned = [];
    const isLearned = (r) => learned.some((l) => key(l.written) === key(r.word) && key(l.heard) === key(r.wrong));

    function showMode() {
      const on = misspelling.checked;
      wrong.classList.toggle('hidden', !on);
      arrow.classList.toggle('hidden', !on);
      right.placeholder = on ? 'Correct spelling' : 'Word, as it should be written';
      right.setAttribute('aria-label', on ? 'Correct spelling' : 'Word');
    }
    function open(i) {
      editing = i;
      confirmedCommon = '';
      const row = i >= 0 ? rows[i] : { word: '', wrong: '' };
      misspelling.checked = !!row.wrong;
      wrong.value = row.wrong;
      right.value = row.word;
      hint.textContent = '';
      submit.textContent = i >= 0 ? 'Save' : 'Add';
      cancel.classList.toggle('hidden', i < 0);
      sheet.classList.toggle('editing', i >= 0);
      showMode();
      (misspelling.checked ? wrong : right).focus();
    }
    const close = () => open(-1);
    showMode();

    function render(typed) {
      const q = key(search.value || (typeof typed === 'string' ? typed : ''));
      list.replaceChildren();
      rows.forEach((r, i) => {
        if (q && !key(r.word + ' ' + r.wrong).includes(q)) return;
        const row = el('div', 'dict-row');
        const what = el('div', 'dict-what');
        what.append(el('span', 'dict-word', r.word));
        const mine = isLearned(r);
        if (mine) what.append(el('span', 'dict-learned', 'learned'));
        const heard = el('span', 'dict-heard', r.wrong || '—');
        const actions = el('span', 'dict-actions');
        const edit = iconButton('Edit', PENCIL);
        edit.addEventListener('click', () => open(i));
        // R-18, R-19: undoing a learned row removes it, and it is never learned again.
        const remove = mine ? el('button', 'dict-undo', 'Undo') : iconButton('Remove', TRASH);
        remove.addEventListener('click', () => {
          // Undoing the last learned misspelling of a word the user added keeps the word.
          const last = !rows.some((o, j) => j !== i && key(o.word) === key(r.word));
          const wordLearned = learned.some((l) => key(l.written) === key(r.word) && !l.heard);
          save(mine && r.wrong && last && !wordLearned
            ? rows.map((o, j) => (j === i ? { word: r.word, wrong: '' } : o))
            : rows.filter((_, j) => j !== i));
        });
        actions.append(edit, remove);
        row.append(what, heard, actions);
        list.append(row);
      });
      head.classList.toggle('hidden', rows.length === 0);
      empty.classList.toggle('hidden', rows.length > 0);
    }
    function showSuggested(list) {
      suggested.replaceChildren();
      suggested.classList.toggle('hidden', !list.length);
      if (!list.length) return;
      for (const s of list) {
        const row = el('div', 'dict-suggestion');
        const what = el('div', 'dict-what');
        const times = s.seen > 1 ? s.seen + ' times' : 'once';
        what.append('You fixed ', el('span', 'dict-wrong-word', s.heard), ' to ', el('span', 'dict-word', s.written),
          ' ' + times + (s.app ? ' in ' + s.app : '') + '. Add it?');
        let warned = false;
        const act = (add) => async () => {
          // R-17: a common misspelling would be replaced everywhere, so it needs a second press.
          if (add && !warned && (await invoke('dictionary_is_common', { word: s.heard }))) {
            warned = true;
            say('“' + s.heard + '” is a common word. HyperMe will replace it everywhere.');
            addButton.textContent = 'Add anyway';
            return;
          }
          try { show(await invoke('dictionary_suggestion', { written: s.written, heard: s.heard, add })); say(''); } catch (e) { say(String(e)); }
        };
        const actions = el('span', 'dict-actions');
        const addButton = el('button', 'main', 'Add');
        addButton.addEventListener('click', act(true));
        const dismiss = el('button', '', 'Not now');
        dismiss.addEventListener('click', act(false));
        actions.append(addButton, dismiss);
        row.append(what, actions);
        suggested.append(row);
      }
    }
    function show(view) {
      learned = view.learned || [];
      learnSwitch.checked = view.learning !== false;
      showSuggested(view.suggestions || []);
      rows = toRows(view.entries);
      where.textContent = 'Saved on this Mac in ' + view.path;
      if (view.error) say(view.error);
      render();
    }
    async function save(next) {
      try {
        show(await invoke('dictionary_save', { entries: toEntries(next) }));
        say('');
        return true;
      } catch (e) {
        say(String(e));
        return false;
      }
    }

    sheet.addEventListener('submit', async (e) => {
      e.preventDefault();
      const word = right.value.trim();
      const heard = misspelling.checked ? wrong.value.trim() : '';
      if (misspelling.checked && !heard) { wrong.focus(); return; }
      if (!word) { right.focus(); return; }
      if (heard.includes(',')) { hint.textContent = 'One misspelling at a time: add each as its own row.'; return; }
      // R-17: a common misspelling would be replaced everywhere, so it needs a second press.
      if (heard && confirmedCommon !== key(heard) && (await invoke('dictionary_is_common', { word: heard }))) {
        confirmedCommon = key(heard);
        hint.textContent = '“' + heard + '” is a common word. HyperMe will replace it everywhere.';
        submit.textContent = 'Add anyway';
        return;
      }
      const next = rows.slice();
      const row = { word, wrong: heard };
      if (editing >= 0) next[editing] = row; else next.unshift(row);
      if (await save(next)) { close(); render(); }
    });
    misspelling.addEventListener('change', () => { showMode(); (misspelling.checked ? wrong : right).focus(); });
    wrong.addEventListener('input', () => { if (confirmedCommon) { confirmedCommon = ''; hint.textContent = ''; submit.textContent = editing >= 0 ? 'Save' : 'Add'; } });
    // The list narrows as the word is typed, so a word already there shows before it is added.
    right.addEventListener('input', () => { if (editing < 0 && !misspelling.checked) render(right.value); });
    cancel.addEventListener('click', close);
    sheet.addEventListener('keydown', (e) => { if (e.key === 'Escape') close(); });
    search.addEventListener('input', render);

    importButton.addEventListener('click', () => file.click());
    file.addEventListener('change', async () => {
      const chosen = file.files[0];
      file.value = '';
      if (!chosen) return;
      try {
        show(await invoke('dictionary_import', { name: chosen.name, text: await chosen.text() }));
        say('');
      } catch (e) { say(String(e)); }
    });
    const exporter = (format) => async () => {
      try { await invoke('dictionary_export', { format }); say(''); } catch (e) { say(String(e)); }
    };
    exportCsv.addEventListener('click', exporter('csv'));
    exportText.addEventListener('click', exporter('text'));

    const load = () => invoke('dictionary_view').then(show).catch((e) => say(String(e)));
    load();
    return { reload: load };
  }

  window.HyperDictionary = { mount, toRows, toEntries };
})();
