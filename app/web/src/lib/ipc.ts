// The Rust commands and events the pages use, typed. Written by hand until the bindings are
// generated from Rust (spec 257, slice 2, R-8 and R-9); the field names follow the Rust structs
// named beside each type.
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

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
}

/** Listens to one of the app's events with its payload typed. */
export function on<E extends keyof Events>(event: E, handler: (payload: Events[E]) => void): Promise<UnlistenFn> {
  return listen<Events[E]>(event, (e) => handler(e.payload));
}
