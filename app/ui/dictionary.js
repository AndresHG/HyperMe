/* The Dictionary page (spec 93, R-10 to R-13, R-15, R-17), shared by Settings → Dictionary and
   the Dictionary page in Home. It works like Wispr Flow's: "Add word" asks for a word; the
   "Correct a misspelling" switch turns it into two fields, the misspelling and the right word.
   Each misspelling is its own row ("zen trix → Zentrix"); a word with no misspelling is a row of
   its own. The whole list is saved on each change through `dictionary_save`.

   HyperDictionary.mount(root, { invoke, onError }) draws the page inside `root`. */
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

  function mount(root, { invoke, onError }) {
    const say = onError || (() => {});
    let rows = [];
    let editing = -1;
    let confirmedCommon = '';

    const bar = el('div', 'dict-bar');
    const search = el('input', 'dict-search');
    search.type = 'search';
    search.placeholder = 'Search your words';
    search.setAttribute('aria-label', 'Search your words');
    const add = el('button', 'main', 'Add word');
    bar.append(search, add);

    const sheet = el('form', 'dict-sheet hidden');
    const switchLabel = el('label', 'dict-switch');
    const misspelling = el('input');
    misspelling.type = 'checkbox';
    misspelling.setAttribute('role', 'switch');
    switchLabel.append(misspelling, el('span', '', 'Correct a misspelling'));
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
    fields.append(wrong, arrow, right);
    const hint = el('p', 'dict-hint');
    const buttons = el('div', 'dict-buttons');
    const cancel = el('button', '', 'Cancel');
    cancel.type = 'button';
    const submit = el('button', 'main', 'Add');
    submit.type = 'submit';
    buttons.append(cancel, submit);
    sheet.append(switchLabel, fields, hint, buttons);

    const list = el('div', 'dict-rows');
    const empty = el('p', 'dict-empty hidden', 'No words yet. Add names, products and jargon, or fix a word from History.');

    const foot = el('div', 'dict-foot');
    const where = el('small', 'dict-where');
    const tools = el('span');
    const importButton = el('button', '', 'Import…');
    const exportCsv = el('button', '', 'Export CSV');
    const exportText = el('button', '', 'Export text');
    tools.append(importButton, ' ', exportCsv, ' ', exportText);
    const file = el('input');
    file.type = 'file';
    file.accept = '.txt,.csv,text/plain,text/csv';
    file.hidden = true;
    foot.append(where, tools, file);

    root.classList.add('dict');
    root.replaceChildren(bar, sheet, list, empty, foot);

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
      sheet.classList.remove('hidden');
      showMode();
      (misspelling.checked ? wrong : right).focus();
    }
    function close() {
      editing = -1;
      sheet.classList.add('hidden');
    }

    function render() {
      const q = key(search.value);
      list.replaceChildren();
      rows.forEach((r, i) => {
        if (q && !key(r.word + ' ' + r.wrong).includes(q)) return;
        const row = el('div', 'dict-row');
        const what = el('div', 'dict-what');
        if (r.wrong) what.append(el('span', 'dict-wrong-word', r.wrong), el('span', 'dict-arrow', '→'));
        what.append(el('span', 'dict-word', r.word));
        const actions = el('span', 'dict-actions');
        const edit = el('button', '', 'Edit');
        edit.addEventListener('click', () => open(i));
        const remove = el('button', '', 'Remove');
        remove.addEventListener('click', () => save(rows.filter((_, j) => j !== i)));
        actions.append(edit, ' ', remove);
        row.append(what, actions);
        list.append(row);
      });
      empty.classList.toggle('hidden', rows.length > 0);
    }
    function show(view) {
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
      if (await save(next)) close();
    });
    misspelling.addEventListener('change', () => { showMode(); (misspelling.checked ? wrong : right).focus(); });
    wrong.addEventListener('input', () => { if (confirmedCommon) { confirmedCommon = ''; hint.textContent = ''; submit.textContent = editing >= 0 ? 'Save' : 'Add'; } });
    add.addEventListener('click', () => open(-1));
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
