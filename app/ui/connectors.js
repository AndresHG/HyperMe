// Settings → Connectors (specs/97-google-connectors, slice 5). Draws what the
// `connectors_view` command returns; every rule lives in Rust. Loaded after settings.html's
// own script, so `invoke` and `$` are already defined there.
(() => {
  let data = null;
  let current = null;

  const el = (tag, attrs = {}, ...children) => {
    const node = document.createElement(tag);
    for (const [k, v] of Object.entries(attrs)) {
      if (k === 'class') node.className = v;
      else if (k.startsWith('on')) node.addEventListener(k.slice(2), v);
      else node.setAttribute(k, v);
    }
    for (const child of children) node.append(child);
    return node;
  };
  const tone = (t) => ({ ok: 'ok', warn: 'warn', bad: 'bad', off: '' })[t] || '';
  const fail = (e) => { $('error').textContent = String(e); };

  function row(r, onclick, words) {
    return el('div', { class: 'row', 'data-words': words, style: 'cursor:pointer', onclick },
      el('div', { class: 'what' }, r.title),
      el('span', {}, el('span', { class: 'state ' + tone(r.tone) }, r.state), ' ›'));
  }

  function showView(which) {
    for (const id of ['connectors-list', 'connectors-sheet', 'connectors-page']) {
      $(id).classList.toggle('hidden', id !== which);
    }
  }

  function drawList() {
    const rows = $('connectors-rows');
    rows.replaceChildren(...data.rows.map((r) =>
      row(r, () => { location.hash = 'connectors/' + r.id; }, 'connector account ' + r.title)));
    $('connectors-empty').classList.toggle('hidden', data.rows.length > 0);
    const setup = $('setup-connectors');
    setup.replaceChildren(...data.setup.map((r) =>
      row(r, () => invoke('connectors_open_link', { url: r.link }).catch(fail), 'connector account setup ' + r.title)));
    $('connectors-choices').replaceChildren(...data.choices.map((c) =>
      el('div', { class: 'row', 'data-words': 'add connector ' + c.name },
        el('div', { class: 'what' }, c.name, el('small', {}, c.note)),
        c.ready ? el('button', { onclick: () => $('connectors-custom').classList.remove('hidden') }, 'Set up')
                : el('button', { disabled: '' }, 'Set up'))));
  }

  function drawPage(page) {
    current = page;
    $('page-title').textContent = page.title;
    const state = $('page-state');
    state.textContent = page.state;
    state.className = 'state ' + tone(page.tone);
    $('page-enabled-row').classList.toggle('hidden', !page.account);
    $('page-enabled').checked = page.enabled;
    $('page-token-row').classList.toggle('hidden', page.account || page.state !== 'Not signed in');
    $('page-unverified').classList.toggle('hidden', page.tools_verified);
    const connector = page.id.split(':')[0];
    $('page-tools').replaceChildren(...page.tools.map((t) => {
      const select = el('select', { 'aria-label': t.name },
        ...[['read', 'Read'], ['ask', 'Ask before'], ['off', 'Off']].map(([v, label]) => {
          const option = el('option', { value: v }, label + (v === t.default ? ' (default)' : ''));
          option.selected = v === t.access;
          return option;
        }));
      select.addEventListener('change', () =>
        invoke('connectors_set_tool', { connector, tool: t.name, access: select.value }).then(refresh).catch(fail));
      return el('div', { class: 'row', 'data-words': 'tool ' + t.name },
        el('div', { class: 'what' }, el('code', {}, t.name)), select);
    }));
    if (!page.tools.length) {
      $('page-tools').replaceChildren(el('p', { class: 'lead' }, 'Tools appear after the first connection.'));
    }
  }

  // Called by settings.html when the hash changes: `item` is the row after connectors/.
  window.connectorsShow = (item) => {
    if (!data || item === undefined) return;
    const page = item && data.pages.find((p) => p.id === item);
    if (page) { drawPage(page); showView('connectors-page'); } else { showView('connectors-list'); }
  };

  async function refresh() {
    data = await invoke('connectors_view');
    if (data.error) fail(data.error);
    drawList();
    const hash = (location.hash || '').slice(1);
    if (hash.startsWith('connectors')) window.connectorsShow(hash.split('/')[1] || null);
  }

  $('connectors-add').addEventListener('click', () => { $('connectors-custom').classList.add('hidden'); showView('connectors-sheet'); });
  $('sheet-close').addEventListener('click', () => showView('connectors-list'));
  $('custom-cancel').addEventListener('click', () => $('connectors-custom').classList.add('hidden'));
  $('custom-add').addEventListener('click', async () => {
    const form = { name: $('custom-name').value, url: $('custom-url').value, command: $('custom-command').value, auth: $('custom-auth').value };
    try {
      const id = await invoke('connectors_add_custom', { form });
      $('error').textContent = '';
      await refresh();
      location.hash = 'connectors/' + id;
    } catch (e) { fail(e); }
  });
  $('page-enabled').addEventListener('change', (e) =>
    invoke('connectors_set_enabled', { account: current.id, enabled: e.target.checked }).then(refresh).catch(fail));
  $('page-token-save').addEventListener('click', async () => {
    const field = $('page-token');
    if (!field.value.trim()) return;
    try {
      await invoke('connectors_set_token', { connector: current.id, token: field.value });
      field.value = '';
      await refresh();
    } catch (e) { fail(e); }
  });

  refresh().catch(fail);
})();
