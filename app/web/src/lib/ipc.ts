// The pages' one door to Rust (spec 257, R-8): the commands and events come from `bindings.ts`,
// which is generated from the Rust commands by tauri-specta (app/src-tauri/src/bindings.rs) and
// checked in; a Rust test fails when it is out of date. This file adds only the short names the
// pages use for a few generated types, and `on` and `tell` over the generated events.
import type { UnlistenFn } from '@tauri-apps/api/event';
import {
  events,
  type HintView,
  type ItemView,
  type LocalServerView,
  type ModelMenuView,
  type PillCard,
  type StatsView,
  type TraceView,
} from './bindings';

export { commands } from './bindings';
export type * from './bindings';

/** The morning card or an alert (pill.rs, `Card`). */
export type Card = PillCard;
/** A dictation in History (home.rs, `ItemView`). */
export type Item = ItemView;
/** The numbers under the greeting (home.rs, `StatsView`). */
export type Stats = StatsView;
/** One item's trace as stage bars (diagnostics.rs, `TraceView`). */
export type Trace = TraceView;
/** A model server found on this Mac (settings.rs, `LocalServerView`). */
export type LocalServer = LocalServerView;
/** The cost and speed line under a model (settings.rs, `HintView`). */
export type ProviderHint = HintView;
/** A provider's models (settings.rs, `ModelMenuView`). */
export type ModelMenu = ModelMenuView;
/** What a task row can do (tasks.rs, `tasks_act`'s `action`). */
export type TaskAction = 'done' | 'tomorrow' | 'drop' | 'keep' | 'rename' | 'move';

/** The app's events by the names Rust sends them under, each the generated event. */
const channels = {
  pill: events.pill,
  'pill-hidden': events.pillHidden,
  card: events.card,
  level: events.level,
  'onboarding-level': events.onboardingLevel,
  'dictation-pasted': events.dictationPasted,
  'providers-changed': events.providersChanged,
  'home://attention': events.homeAttention,
  'home://refresh': events.homeRefresh,
  'home://changed': events.homeChanged,
  'home://retry': events.homeRetry,
  'connector-approval': events.connectorApproval,
  'connector-approval-closed': events.connectorApprovalClosed,
};
type Channels = typeof channels;
/** The payload of one of the app's events, as the generated bindings type it. */
export type Payload<E extends keyof Channels> = Parameters<Parameters<Channels[E]['listen']>[0]>[0]['payload'];

/** Listens to one of the app's events with its payload typed. */
export function on<E extends keyof Channels>(event: E, handler: (payload: Payload<E>) => void): Promise<UnlistenFn> {
  const subscribe = channels[event].listen as (cb: (e: { payload: Payload<E> }) => void) => Promise<UnlistenFn>;
  return subscribe((e) => handler(e.payload));
}

/** Events the pages send to Rust. */
export const tell = {
  /** A Home block (or `today`) was on screen, which clears what was new there (R-21, R-22). */
  blockViewed: (id: string) => events.homeBlockViewed.emit(id),
};
