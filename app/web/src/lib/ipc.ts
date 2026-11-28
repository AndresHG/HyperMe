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

// ---- Settings (settings.rs, connectors.rs, diagnostics.rs, report.rs, approvals.rs) ----

/** The config values Settings shows (settings.rs, `ConfigView`). */
export interface ConfigView {
  hotkey: string;
  min_hold_ms: number;
  max_length_s: number;
  short_dictation_words: number;
  sounds: boolean;
  show_while_idle: boolean;
  mute_while_dictating: boolean;
  speech_provider: string;
  speech_model: string;
  speech_endpoint: string;
  cleanup_provider: string;
  cleanup_model: string;
  cleanup_endpoint: string;
  reasoning_effort: string;
  deadline_ms: number;
  history: boolean;
  retention_days: number | null;
  size_limit_mb: number;
  keep_traces: boolean;
  timing_log: boolean;
}

/** A model in a provider's catalogue (settings.rs, `ModelView`). */
export interface ModelView {
  id: string;
  reasoning_effort: string[];
}

/** A provider in a role's menu, never with its key (settings.rs, `ProviderView`). */
export interface ProviderView {
  id: string;
  name: string;
  recommended: boolean;
  key_required: boolean;
  has_key: boolean;
  has_key_page: boolean;
  key_note: string | null;
  models: ModelView[];
}

/** A role's fallback (settings.rs, `FallbackView`). */
export interface FallbackView {
  provider: string;
  name: string;
  model: string;
  ready: boolean;
}

/** One row of the providers table (settings.rs, `RoleView`). */
export interface RoleView {
  role: Role;
  label: string;
  provider: string;
  model: string;
  reasoning_effort: string | null;
  providers: ProviderView[];
  fallback: FallbackView | null;
}

/** Where dictation may send something (config.rs, `Destination`). */
export interface Destination {
  role: Role;
  name: string;
  model: string;
  fallback_for: string | null;
  on_this_mac: boolean;
}

/** What Settings shows (settings.rs, `View`). Never holds a key. */
export interface SettingsView {
  config: ConfigView | null;
  config_error: string | null;
  config_path: string;
  keys: { speech: boolean; cleanup: boolean };
  roles: RoleView[];
  destinations: Destination[];
  setup: { accessibility: boolean; input_monitoring: boolean; host_app: string | null };
  version: string;
}

/** The microphone order (settings.rs, `MicrophonesView`). */
export interface MicrophonesView {
  ordered: boolean;
  microphones: { name: string; connected: boolean; default: boolean }[];
}

/** A provider's live model list (settings.rs, `ModelMenuView`). */
export interface ModelMenu {
  models: string[];
  notice: string | null;
}

/** Cost and speed under a role (settings.rs, `HintView`). */
export interface ProviderHint {
  cost: string;
  speed: string;
  source: string | null;
  mine: string | null;
}

/** A model server found on this Mac (settings.rs, `LocalServerView`). */
export interface LocalServer {
  port: number;
  endpoint: string;
  models: string[];
}

export type Tone = 'ok' | 'warn' | 'bad' | 'off';

/** A connector account's row (connector_settings.rs, `Row`). */
export interface ConnectorRow {
  id: string;
  connector: string;
  title: string;
  state: string;
  tone: Tone;
  link: string;
}

export type ToolAccess = 'read' | 'ask' | 'off';

/** A line on a connector page about a server's approvals (connectors.rs, `ApprovalLine`). */
export interface ApprovalLine {
  server: string;
  text: string;
  action: string | null;
}

/** A connector account's page (connectors.rs, `Page`). */
export interface ConnectorPage {
  id: string;
  title: string;
  state: string;
  tone: Tone;
  account: boolean;
  enabled: boolean;
  oauth: boolean;
  label: string;
  features: string[];
  tools_verified: boolean;
  tools: { name: string; access: ToolAccess; default: ToolAccess; changed: boolean }[];
  approvals: ApprovalLine[];
  requests: string[];
}

/** The two-step connect dialog of an OAuth preset (connectors.rs, `SetupSheet`). */
export interface SetupSheet {
  connector: string;
  name: string;
  client_saved: boolean;
  client_id: string | null;
  client_url: string | null;
  production_url: string | null;
  commands: string[];
  features: { id: string; label: string; on_by_default: boolean }[];
}

/** Settings → Connectors (connectors.rs, `View`). */
export interface ConnectorsView {
  rows: ConnectorRow[];
  setup: ConnectorRow[];
  choices: { id: string; name: string; ready: boolean; note: string }[];
  privacy: {
    accounts: { title: string; reads: string[]; approvals: string }[];
    where_it_goes: string;
    kept: string;
  } | null;
  sheets: SetupSheet[];
  pages: ConnectorPage[];
  error: string | null;
}

/** A finished sign-in (connectors.rs, `SignInResult`). */
export interface SignInResult {
  account: string;
  checks: string[];
}

/** A run of text in an approval message (approval.rs, `Span`). */
export interface Span {
  kind: 'text' | 'bold' | 'italic' | 'code' | 'break' | 'item' | 'link';
  text: string;
  url?: string;
}

/** What a connector server asks (approvals.rs, `CardView`). */
export interface ApprovalCard {
  id: number;
  who: string;
  title: string;
  message: Span[];
  choices: { label: string; value: unknown }[];
  default: number | null;
  note: string | null;
}

/** Percentiles over a window (diagnostics.rs, `Window`). */
export type Window =
  | { state: 'too_few'; count: number }
  | { state: 'ready'; count: number; p50_us: number; p90_us: number; over_budget: boolean };

/** Settings → Diagnostics (diagnostics.rs, `DiagnosticsView`). */
export interface DiagnosticsView {
  shown: boolean;
  keep: boolean;
  view: {
    empty: boolean;
    last_day: Window;
    last_week: Window;
    stages: { stage: string; provider: string | null; model: string | null; window: Window }[];
    outcomes: [string, number][];
    failures: [string, number][];
    slowest_stage: string | null;
  } | null;
  warnings: string[];
  error: string | null;
}

/** Report a problem's bundle (report.rs, `ReportView`). */
export interface ReportView {
  text: string;
  trimmed: string[];
  url: string;
  has_reports: boolean;
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
  keyTest: (role: Role, provider?: string, model?: string) => invoke<number>('key_test', { role, provider, model }),
  keySet: (role: Role, key: string, provider?: string) => invoke<void>('key_set', { role, key, provider }),
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
  settingsView: () => invoke<SettingsView>('settings_view'),
  settingsOpen: (what: string) => invoke<void>('settings_open', { what }),
  appRestart: () => invoke<void>('app_restart'),
  microphonesList: () => invoke<MicrophonesView>('microphones_list'),
  microphonesSet: (names: string[]) => invoke<void>('microphones_set', { names }),
  providerChoose: (role: Role, provider: string, model: string) =>
    invoke<'saved' | 'needs_key'>('provider_choose', { role, provider, model }),
  providerAdd: (name: string, endpoint: string) => invoke<string>('provider_add', { name, endpoint }),
  providerHint: (role: Role, provider: string, model: string) => invoke<ProviderHint>('provider_hint', { role, provider, model }),
  providerKeyPage: (provider: string) => invoke<void>('provider_key_page', { provider }),
  providersReset: () => invoke<void>('providers_reset'),
  modelsList: (provider: string) => invoke<ModelMenu>('models_list', { provider }),
  localDiscover: () => invoke<LocalServer[]>('local_discover'),
  localUse: (endpoint: string, model: string) => invoke<void>('local_use', { endpoint, model }),
  fallbackSet: (role: Role, provider: string, model: string) => invoke<number>('fallback_set', { role, provider, model }),
  fallbackClear: (role: Role) => invoke<void>('fallback_clear', { role }),
  resultOpen: (source: string) => invoke<void>('result_open', { source }),
  connectorsView: () => invoke<ConnectorsView>('connectors_view'),
  connectorsOpenLink: (url: string) => invoke<void>('connectors_open_link', { url }),
  connectorsOpenUrl: (connector: string, which: 'client' | 'production') => invoke<void>('connectors_open_url', { connector, which }),
  connectorsReview: (account: string, server: string) => invoke<string>('connectors_review', { account, server }),
  connectorsSetTool: (connector: string, tool: string, access: ToolAccess) =>
    invoke<void>('connectors_set_tool', { connector, tool, access }),
  connectorsAnswer: (id: number, value: unknown) => invoke<boolean>('connectors_answer', { id, value }),
  connectorsAddCustom: (form: { name: string; url: string; command: string; auth: string }) =>
    invoke<string>('connectors_add_custom', { form }),
  connectorsSetEnabled: (account: string, enabled: boolean) => invoke<void>('connectors_set_enabled', { account, enabled }),
  connectorsSetToken: (connector: string, token: string) => invoke<void>('connectors_set_token', { connector, token }),
  connectorsSaveClient: (connector: string, clientId: string, clientSecret: string) =>
    invoke<number>('connectors_save_client', { connector, clientId, clientSecret }),
  connectorsSignIn: (connector: string, label: string, features: string[]) =>
    invoke<SignInResult>('connectors_sign_in', { connector, label, features }),
  connectorsCancelSignIn: () => invoke<void>('connectors_cancel_sign_in'),
  connectorsDisconnect: (account: string) => invoke<void>('connectors_disconnect', { account }),
  diagnosticsView: () => invoke<DiagnosticsView>('diagnostics_view'),
  reportBuild: (description: string, includeReports: boolean, fromItem: number | null) =>
    invoke<ReportView>('report_build', { description, includeReports, fromItem }),
  reportSave: (text: string) => invoke<string | null>('report_save', { text }),
  reportOpenIssue: (text: string, url: string) => invoke<string>('report_open_issue', { text, url }),
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
  /** A connector server asks something (specs/166). */
  'connector-approval': ApprovalCard;
  /** The question with this id is over. */
  'connector-approval-closed': number;
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
