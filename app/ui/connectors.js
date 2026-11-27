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
    for (const id of ['connectors-list', 'connectors-sheet', 'connectors-page', 'connectors-oauth']) {
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
        c.ready ? el('button', { onclick: () => (c.id === 'custom' ? $('connectors-custom').classList.remove('hidden') : openSheet(c.id)) }, 'Set up')
                : el('button', { disabled: '' }, 'Set up'))));
    drawPrivacy();
  }

  function drawPrivacy() {
    const p = data.privacy;
    $('privacy-connectors').classList.toggle('hidden', !p || !p.accounts.length);
    if (!p) return;
    $('privacy-accounts').replaceChildren(...p.accounts.map((a) =>
      el('div', { class: 'row', 'data-words': 'connected account ' + a.title },
        el('div', { class: 'what' }, a.title, el('small', {}, a.reads.join(' · ')),
          el('small', {}, a.approvals || '')))));
    $('privacy-where').textContent = p.where_it_goes;
    $('privacy-kept').textContent = p.kept;
  }

  // The setup sheet of an OAuth preset: client, then sign-in with a label and switches.
  let sheet = null;
  function openSheet(connector, label = '', features = null) {
    sheet = data.sheets.find((s) => s.connector === connector);
    if (!sheet) { fail('No setup sheet for ' + connector); return; }
    $('oauth-title').textContent = 'Add ' + sheet.name;
    $('oauth-client-id').value = sheet.client_id || '';
    $('oauth-client-secret').value = '';
    const state = $('oauth-client-state');
    state.textContent = sheet.client_saved ? 'Client saved' : 'No client yet';
    state.className = 'state ' + (sheet.client_saved ? 'ok' : 'warn');
    $('oauth-label').value = label;
    $('oauth-label').disabled = !!label;
    $('oauth-features').replaceChildren(...sheet.features.map((f) => {
      const box = el('input', { type: 'checkbox', 'data-feature': f.id });
      box.checked = features ? features.includes(f.id) : f.on_by_default;
      return el('div', { class: 'row', 'data-words': 'scope ' + f.label }, el('div', { class: 'what' }, f.label), box);
    }));
    $('oauth-signin').classList.toggle('hidden', !sheet.client_saved);
    for (const id of ['oauth-testing', 'oauth-done', 'oauth-cancel']) $(id).classList.add('hidden');
    $('oauth-wait').textContent = '';
    $('oauth-go').disabled = false;
    showView('connectors-oauth');
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
    $('page-approvals').replaceChildren(...(page.approvals || []).map((line) =>
      el('div', { class: 'row', 'data-words': 'approval needs your ok not allowed blocked held back' }, el('div', { class: 'what' }, el('small', {}, line)))));
    $('page-requests').replaceChildren(...(page.requests || []).map((line) =>
      el('div', { class: 'row', 'data-words': 'requests quota today' }, el('div', { class: 'what' }, el('small', {}, line)))));
    $('page-oauth-row').classList.toggle('hidden', !(page.account && page.oauth));
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

  $('oauth-open-client').addEventListener('click', () => invoke('connectors_open_url', { connector: sheet.connector, which: 'client' }).catch(fail));
  $('oauth-open-production').addEventListener('click', () => invoke('connectors_open_url', { connector: sheet.connector, which: 'production' }).catch(fail));
  $('oauth-gcloud').addEventListener('click', async () => {
    await navigator.clipboard.writeText(sheet.commands.join('\n'));
    $('oauth-client-state').textContent = 'gcloud commands copied; the OAuth client itself is made on the setup page';
  });
  $('oauth-client-save').addEventListener('click', async () => {
    const state = $('oauth-client-state');
    state.textContent = 'Testing…';
    state.className = 'state warn';
    try {
      const ms = await invoke('connectors_save_client', { connector: sheet.connector, clientId: $('oauth-client-id').value, clientSecret: $('oauth-client-secret').value });
      $('oauth-client-secret').value = '';
      state.textContent = 'Works, ' + ms + ' ms';
      state.className = 'state ok';
      await refresh();
      sheet = data.sheets.find((s) => s.connector === sheet.connector);
      $('oauth-signin').classList.remove('hidden');
    } catch (e) { state.textContent = String(e); state.className = 'state bad'; }
  });
  $('oauth-go').addEventListener('click', async () => {
    const features = [...document.querySelectorAll('[data-feature]')].filter((b) => b.checked).map((b) => b.dataset.feature);
    $('oauth-wait').textContent = 'Finish in your browser…';
    $('oauth-wait').className = 'state warn';
    $('oauth-go').disabled = true;
    $('oauth-cancel').classList.remove('hidden');
    try {
      const result = await invoke('connectors_sign_in', { connector: sheet.connector, label: $('oauth-label').value, features });
      await refresh();
      $('oauth-wait').textContent = '';
      $('oauth-done').replaceChildren(el('p', { class: 'ok' }, 'Signed in as ' + result.account + '.'), ...result.checks.map((c) => el('p', { class: 'lead' }, c)));
      $('oauth-done').classList.remove('hidden');
    } catch (e) {
      $('oauth-wait').textContent = e.message || String(e);
      $('oauth-wait').className = 'state bad';
      $('oauth-testing').classList.toggle('hidden', !e.testing);
    } finally {
      $('oauth-go').disabled = false;
      $('oauth-cancel').classList.add('hidden');
    }
  });
  $('oauth-cancel').addEventListener('click', () => invoke('connectors_cancel_sign_in'));
  $('oauth-close').addEventListener('click', () => { location.hash = 'connectors'; showView('connectors-list'); });
  $('page-sign-in').addEventListener('click', () => openSheet(current.id.split(':')[0], current.label, current.features));
  $('page-disconnect').addEventListener('click', async () => {
    if (!confirm('Disconnect ' + current.title + '? This removes access and deletes what HyperMe cached from this account.')) return;
    try {
      await invoke('connectors_disconnect', { account: current.id });
      location.hash = 'connectors';
      await refresh();
    } catch (e) { fail(e); await refresh(); }
  });

  refresh().catch(fail);
})();
