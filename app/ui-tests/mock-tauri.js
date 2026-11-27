// A stand-in for Tauri's API in a plain browser, for the screenshot checks: commands answer from
// these fixtures (synthetic data only, never a real dictation), events never fire. A command
// missing here answers null.
(() => {
  const now = Date.UTC(2026, 10, 27, 9, 30);
  const item = (id, minutes, text, app) => ({ id, started_ms: now - minutes * 60000, app, language: 'en', text, state: 'done', message: null, has_audio: true, actions: [{ id: 'copy', label: 'Copy', key: 'C' }, { id: 'delete', label: 'Delete', key: '⌫' }] });
  const fixtures = {
    home_info: { history: true, dictionary: true },
    home_sections: [
      { id: 'home', title: 'Home', page: 'home.html', shortcut: '⌘1' },
      { id: 'history', title: 'History', page: 'home.html', shortcut: '⌘2' },
      { id: 'dictionary', title: 'Dictionary', page: 'home-dictionary.html', shortcut: '⌘3' },
    ],
    home_blocks: [],
    home_attention: false,
    home_stats: { words_this_week: 1840, seconds_saved: 2460 },
    home_page: [item(3, 12, 'Send the weekly update to the team before lunch.', 'Mail'), item(2, 95, 'Remind me to review the design tokens pull request.', 'Slack'), item(1, 1500, 'The quick brown fox jumps over the lazy dog.', 'Notes')],
    home_search: [],
    home_failed: [],
    home_today: {
      lines: [
        { id: 1, title: 'Review the design tokens pull request', marks: ['new'], rolled: 0 },
        { id: 2, title: 'Send the weekly update', marks: ['due today'], rolled: 0 },
        { id: 3, title: 'Book the team offsite room', marks: [], rolled: 0 },
      ],
      done_today: 2, waiting: [], tomorrow: '2026-11-28',
      meetings: [
        { title: 'Stand-up', start: '09:30', end: '09:45', past: true, now: false, starts_in_min: null, link: null },
        { title: 'Design review', start: '11:00', end: '12:00', past: false, now: false, starts_in_min: 90, link: 'https://example.com' },
      ],
    },
    dictionary_suggested: 2,
    dictionary_view: { entries: [{ written: 'HyperMe', heard: ['hyper me', 'hipermi'] }, { written: 'Tauri', heard: [] }, { written: 'Kubernetes', heard: ['cooper netties'] }], suggestions: [], learning: true, path: '~/Library/Application Support/HyperMe/dictionary.toml' },
    settings_pending: null,
    microphones_list: { ordered: false, microphones: [
      { name: 'MacBook Pro Microphone', connected: true, default: true },
      { name: 'Studio USB Microphone', connected: true, default: false },
    ] },
    onboarding_checks: { accessibility: true, input_monitoring: false, host_app: null, bundled: true, speech_provider: 'groq', cleanup_provider: 'groq' },
    onboarding_can_continue: true,
    onboarding_keys_screen: { speech: 'Groq', cleanup: 'Groq', free_credit: 'Groq has a free tier.', other_keys: [], choices: [
      { label: 'Light', minutes_per_day: 10, detail: '10 minutes a day, about', shown_as: 'About $0.40 a month', speech_usd: 0.3, cleanup_usd: 0.1 },
      { label: 'Daily', minutes_per_day: 30, detail: '30 minutes a day, about', shown_as: 'About $1.20 a month', speech_usd: 0.9, cleanup_usd: 0.3 },
      { label: 'Heavy', minutes_per_day: 90, detail: '90 minutes a day, about', shown_as: 'About $3.60 a month', speech_usd: 2.7, cleanup_usd: 0.9 } ] },
    settings_view: {
      config: { hotkey: 'fn', min_hold_ms: 150, max_length_s: 300, short_dictation_words: 4, sounds: true, show_while_idle: true, speech_provider: 'groq', speech_model: 'whisper-large-v3-turbo', speech_endpoint: '', cleanup_provider: 'groq', cleanup_model: 'llama-3.3-70b-versatile', cleanup_endpoint: '', reasoning_effort: 'none', deadline_ms: 800, history: true, retention_days: 30, size_limit_mb: 500, keep_traces: true, timing_log: false },
      config_error: null, config_path: '~/Library/Application Support/HyperMe/config.toml',
      keys: { speech: true, cleanup: true }, roles: [], destinations: [],
      setup: { accessibility: true, input_monitoring: true, host_app: null }, version: '0.1.0',
    },
    connectors_view: { rows: [], setup: [], choices: [{ id: 'google', name: 'Google', note: 'Mail and calendar', ready: true }], privacy: null, pages: [], sheets: {} },
    diagnostics_view: { stages: [], warnings: '' },
  };
  window.__mockCalls = [];
  window.__TAURI__ = {
    core: {
      invoke: async (cmd, args) => {
        window.__mockCalls.push(cmd);
        const f = fixtures[cmd];
        return typeof f === 'function' ? f(args) : structuredClone(f === undefined ? null : f);
      },
    },
    event: { listen: async () => () => {}, emit: async () => {} },
  };
  // The same stand-in for pages built with @tauri-apps/api (app/web, spec 257), which call
  // Tauri's internals. Events fire only when a check sends one with `window.__mockEmit`.
  const callbacks = new Map();
  const listeners = new Map();
  let next = 1;
  window.__TAURI_INTERNALS__ = {
    transformCallback: (callback) => { const id = next++; callbacks.set(id, callback); return id; },
    unregisterCallback: (id) => callbacks.delete(id),
    invoke: async (cmd, args) => {
      if (cmd === 'plugin:event|listen') {
        listeners.set(args.event, [...(listeners.get(args.event) || []), args.handler]);
        return args.handler;
      }
      if (cmd.startsWith('plugin:event|')) return null;
      return window.__TAURI__.core.invoke(cmd, args);
    },
  };
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
  window.__mockEmit = (event, payload) => {
    for (const id of listeners.get(event) || []) callbacks.get(id)?.({ event, id, payload });
  };
})();
