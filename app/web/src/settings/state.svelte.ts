// What every Settings section shares: the settings view from Rust, the connectors view, the error
// line, the restart banner and where the sheet is. One instance per opening of the sheet, given to
// the sections through context, so each opening starts fresh as the old page did.
import { getContext, setContext } from 'svelte';
import { SvelteMap } from 'svelte/reactivity';
import { closeSettings, sheet } from '../home/settings.svelte';
import { commands, type ConnectorsView, type Role, type SettingsView } from '../lib/ipc';

/** A state line: its words and its colour class (`ok`, `bad`, `warn`, or none). */
export interface Mark {
  text: string;
  tone: '' | 'ok' | 'bad' | 'warn';
}

export const mark = (ok: boolean, yes: string, no: string): Mark => ({ text: ok ? yes : no, tone: ok ? 'ok' : 'bad' });
export const testing: Mark = { text: 'Testing…', tone: 'warn' };
export const noMark: Mark = { text: '', tone: '' };

/** The connectors view's tone as a class (`off` draws plain). */
export const tone = (t: string) => (t === 'ok' || t === 'warn' || t === 'bad' ? t : '');

export class Settings {
  view = $state<SettingsView | null>(null);
  connectors = $state<ConnectorsView | null>(null);
  error = $state('');
  /** A saved setting needs a restart. */
  banner = $state(false);
  /** Show advanced: the Advanced section. */
  advanced = $state(false);
  /** A provider picked without a key waits here, per role, until its key passes Test (R-12). */
  pending = $state<Partial<Record<Role, { provider: string; model: string }>>>({});
  /** The last Review line per connector account and server, kept across redraws (specs/166). */
  readonly reviewed = new SvelteMap<string, string>();

  /** The section shown, and the item after its slash (`connectors/<id>`). */
  readonly section = $derived.by(() => {
    const [id = 'setup', item] = sheet.place.split('/');
    // The old Providers places (the walkthrough, menus, the "keeps failing" notice) land on the
    // AI models pages: cleanup on Language model, the rest on Speech to text.
    if (id === 'providers' || id === 'providers-advanced') return item === 'cleanup' ? 'language' : 'speech';
    return id;
  });
  readonly item = $derived(sheet.place.includes('/') ? sheet.place.slice(sheet.place.indexOf('/') + 1) : null);

  /** Goes to a place inside the sheet (`privacy`, `connectors/<id>`). */
  go(place: string) {
    sheet.place = place;
    sheet.visit++;
  }

  close() {
    closeSettings();
  }

  fail = (e: unknown) => {
    this.error = String(e);
  };

  async refresh() {
    try {
      this.view = await commands.settingsView();
      this.error = this.view.config_error ?? '';
    } catch (e) {
      this.fail(e);
    }
  }

  async refreshConnectors() {
    this.connectors = await commands.connectorsView();
    if (this.connectors.error) this.fail(this.connectors.error);
  }

  async save(section: string, key: string, value: string | number | boolean) {
    try {
      await commands.settingsSet(section, key, value);
      this.error = '';
      // Provider settings reach the next dictation at once (R-8); the rest need a restart.
      // So does muting while dictating (#221).
      const live = section === 'speech' || section === 'cleanup' || key === 'mute_while_dictating';
      if (!live) this.banner = true;
    } catch (e) {
      this.fail(e);
      this.refresh();
    }
  }
}

const KEY = Symbol('settings');
export const provideSettings = (s: Settings) => setContext(KEY, s);
export const useSettings = () => getContext<Settings>(KEY);
