// The Rust commands and events the pages use, typed. Written by hand until the bindings are
// generated from Rust (spec 257, slice 2, R-8 and R-9); the field names follow the Rust structs
// named beside each type.
import { invoke } from '@tauri-apps/api/core';
import { emit, listen, type UnlistenFn } from '@tauri-apps/api/event';

/** One state of the pill (pill.rs, `View`). `kind` `card` asks the page to fetch the card. */
export interface PillView {
  kind: string;
  label: string;
  hide_after_ms?: number | null;
  retry?: boolean;
}

/** A line of the morning card (pill.rs, `CardLine`). */
export interface CardLine {
  title: string;
  mark: string | null;
  tone: string | null;
}

/** The morning card (pill.rs, `CardView`). */
export interface MorningCard {
  kind?: 'morning';
  title: string;
  yesterday: string | null;
  meetings: string | null;
  tasks: CardLine[];
  footer: string | null;
  mail: string | null;
  stale: string | null;
}

/** An alert in the card (pill.rs, `AlertView`). */
export interface AlertCard {
  kind: 'alert';
  title: string;
  detail: string;
  action: string;
  link: string | null;
}

export type Card = MorningCard | AlertCard;

/** The walkthrough's steps (onboarding.rs, `Step`). */
export type Step = 'welcome' | 'microphone' | 'permissions' | 'keys' | 'try';

/** What the walkthrough has seen work (onboarding.rs, `Checks`). */
export interface Checks {
  microphone: boolean;
  accessibility: boolean;
  input_monitoring: boolean;
  speech_key: boolean;
  cleanup_key: boolean;
  tried: boolean;
}

/** The permission checks (walkthrough.rs, `Permissions`). */
export interface Permissions {
  accessibility: boolean;
  input_monitoring: boolean;
  host_app: string | null;
  bundled: boolean;
  speech_provider: string;
  cleanup_provider: string;
}

/** One answer to "How much do you talk a day?" (walkthrough.rs, `CostChoice`). */
export interface CostChoice {
  label: string;
  minutes_per_day: number;
  detail: string;
  shown_as: string;
  speech_usd: number;
  cleanup_usd: number;
}

/** The keys screen (walkthrough.rs, `KeysScreen`). */
export interface KeysScreen {
  speech: string;
  cleanup: string;
  choices: CostChoice[];
  free_credit: string | null;
  other_keys: string[];
}

export type Role = 'speech' | 'cleanup';

/** A sidebar section (home.rs, `Section`). */
export interface Section {
  id: string;
  title: string;
  page: string;
  shortcut: string;
}

/** A Home-page block from another feature (home.rs, `Block`). */
export interface Block {
  id: string;
  title: string;
}

/** What Home needs to know once (home.rs, `Info`). */
export interface HomeInfo {
  history: boolean;
  dictionary: boolean;
}

/** The numbers under the greeting (home.rs, `StatsView`). */
export interface Stats {
  words_this_week: number;
  seconds_saved: number;
}

/** One action a History row offers, with its key (home.rs, `Action`). */
export interface ItemAction {
  id: 'copy' | 'paste' | 'retry' | 'fix' | 'delete';
  label: string;
  key: string;
}

/** A dictation in History (home.rs, `ItemView`). */
export interface Item {
  id: number;
  started_ms: number;
  app: string | null;
  language: string | null;
  text: string | null;
  state: string;
  message: string | null;
  has_audio: boolean;
  actions: ItemAction[];
}

/** Paste again (home.rs, `PasteOutcome`). */
export interface PasteOutcome {
  pasted: boolean;
  notice: string | null;
}

/** Fix a word (home.rs, `FixOutcome`). */
export type FixOutcome = { outcome: 'fixed'; text: string } | { outcome: 'needs_confirm'; heard: string };

/** One item's trace as stage bars (diagnostics.rs, `TraceView`). */
export interface Trace {
  bars: { stage: string; start_ms: number; duration_ms: number }[];
  speech: [string, string] | null;
  cleanup: [string, string] | null;
  outcome: string;
}

/** Today (tasks.rs, `TodayPage`). */
export interface TodayPage {
  today: string;
  tomorrow: string;
  meetings: MeetingRow[];
  lines: LineRow[];
  waiting: WaitingRow[];
  done_today: number;
  has_new: boolean;
}

export interface MeetingRow {
  title: string;
  start: string;
  end: string;
  link: string | null;
  starts_in_min: number | null;
  now: boolean;
  past: boolean;
}

export interface LineRow {
  id: number;
  title: string;
  marks: string[];
  stale: boolean;
  rolled: number;
}

export interface WaitingRow {
  id: number;
  title: string;
  on: string;
  new: boolean;
}

export type TaskAction = 'done' | 'tomorrow' | 'drop' | 'keep' | 'rename' | 'move';

/** A dictionary word and what it is often heard as (dictionary.rs, `Entry`). */
export interface DictionaryEntry {
  written: string;
  heard: string[];
}

/** The Dictionary page (dictionary.rs, `View`). */
export interface DictionaryView {
  path: string;
  entries: DictionaryEntry[];
  error: string | null;
  suggestions: { written: string; heard: string; seen: number; app: string | null }[];
  learned: { written: string; heard: string }[];
  learning: boolean;
}

/** A retry's progress (runtime.rs, `home://retry`). */
export interface RetryEvent {
  id: number;
  state: 'working' | 'done' | 'failed' | string;
  message: string | null;
}

export const commands = {
  pillView: () => invoke<PillView | null>('pill_view'),
  pillCard: () => invoke<Card | null>('pill_card'),
  pillClicked: () => invoke<void>('pill_clicked'),
  pillHandsFree: (stop: boolean) => invoke<void>('pill_hands_free', { stop }),
  cardOpenToday: () => invoke<void>('card_open_today'),
  cardAction: () => invoke<void>('card_action'),
  cardDismiss: () => invoke<void>('card_dismiss'),
  onboardingCanContinue: (step: Step, checks: Checks) => invoke<boolean>('onboarding_can_continue', { step, checks }),
  onboardingChecks: () => invoke<Permissions>('onboarding_checks'),
  onboardingMic: (on: boolean) => invoke<void>('onboarding_mic', { on }),
  onboardingRequest: (what: string, prompt: boolean) => invoke<void>('onboarding_request', { what, prompt }),
  onboardingRelaunch: (step: Step) => invoke<void>('onboarding_relaunch', { step }),
  onboardingTry: () => invoke<void>('onboarding_try'),
  onboardingKeysScreen: () => invoke<KeysScreen>('onboarding_keys_screen'),
  onboardingGetKey: (role: Role) => invoke<void>('onboarding_get_key', { role }),
  onboardingOtherProviders: () => invoke<void>('onboarding_other_providers'),
  onboardingDone: () => invoke<void>('onboarding_done'),
  keyTest: (role: Role) => invoke<number>('key_test', { role }),
  keySet: (role: Role, key: string) => invoke<void>('key_set', { role, key }),
  settingsSet: (section: string, key: string, value: unknown) => invoke<void>('settings_set', { section, key, value }),
  homeSections: () => invoke<Section[]>('home_sections'),
  homeBlocks: () => invoke<Block[]>('home_blocks'),
  homeAttention: (on: boolean | null) => invoke<boolean>('home_attention', { on }),
  homeInfo: () => invoke<HomeInfo>('home_info'),
  homeStats: (utcOffsetMinutes: number) => invoke<Stats>('home_stats', { utcOffsetMinutes }),
  homePage: (before: [number, number] | null, limit: number) => invoke<Item[]>('home_page', { before, limit }),
  homeSearch: (query: string, before: [number, number] | null, limit: number) =>
    invoke<Item[]>('home_search', { query, before, limit }),
  homeFailed: () => invoke<Item[]>('home_failed'),
  homeItem: (id: number) => invoke<Item | null>('home_item', { id }),
  homeCopy: (id: number) => invoke<void>('home_copy', { id }),
  homePaste: (id: number) => invoke<PasteOutcome>('home_paste', { id }),
  homeRetry: (id: number) => invoke<void>('home_retry', { id }),
  homeDelete: (id: number) => invoke<void>('home_delete', { id }),
  homeFixWord: (id: number, heard: string, wanted: string, confirmedCommon: boolean) =>
    invoke<FixOutcome>('home_fix_word', { id, heard, wanted, confirmedCommon }),
  historyTrace: (id: number) => invoke<Trace | null>('history_trace', { id }),
  homeToday: () => invoke<TodayPage>('home_today'),
  tasksAdd: (title: string) => invoke<void>('tasks_add', { title }),
  tasksAct: (id: number, action: TaskAction, value: string | null = null) => invoke<void>('tasks_act', { id, action, value }),
  tasksOpenMeeting: (url: string) => invoke<void>('tasks_open_meeting', { url }),
  dictionaryView: () => invoke<DictionaryView>('dictionary_view'),
  dictionarySave: (entries: DictionaryEntry[]) => invoke<DictionaryView>('dictionary_save', { entries }),
  dictionaryIsCommon: (word: string) => invoke<boolean>('dictionary_is_common', { word }),
  dictionaryImport: (name: string, text: string) => invoke<DictionaryView>('dictionary_import', { name, text }),
  dictionaryExport: (format: 'csv' | 'text') => invoke<string>('dictionary_export', { format }),
  dictionarySuggestion: (written: string, heard: string, add: boolean) =>
    invoke<DictionaryView>('dictionary_suggestion', { written, heard, add }),
  dictionaryLearning: (on: boolean | null) => invoke<boolean>('dictionary_learning', { on }),
  dictionarySuggested: () => invoke<number>('dictionary_suggested'),
  settingsPending: () => invoke<string | null>('settings_pending'),
  settingsOpened: () => invoke<void>('settings_opened'),
  settingsClosed: () => invoke<void>('settings_closed'),
};

export interface Events {
  pill: PillView;
  'pill-hidden': null;
  card: Card;
  level: number;
  /** The walkthrough's microphone level and whether a voice was heard. */
  'onboarding-level': [number, boolean];
  /** A dictation was pasted, with its time from key release in ms. */
  'dictation-pasted': number;
  'providers-changed': null;
  'home://attention': boolean;
  'home://refresh': null;
  'home://changed': null;
  'home://retry': RetryEvent;
}

/** Listens to one of the app's events with its payload typed. */
export function on<E extends keyof Events>(event: E, handler: (payload: Events[E]) => void): Promise<UnlistenFn> {
  return listen<Events[E]>(event, (e) => handler(e.payload));
}

/** Events the pages send to Rust. */
export const tell = {
  /** A Home block (or `today`) was on screen, which clears what was new there (R-21, R-22). */
  blockViewed: (id: string) => emit('home://block-viewed', id),
};
