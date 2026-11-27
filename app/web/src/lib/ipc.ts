// The Rust commands and events the pages use, typed. Written by hand for the pill until the
// bindings are generated from Rust (spec 257, slice 2, R-8 and R-9); the field names follow the
// Rust structs in app/src-tauri/src/pill.rs.
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

export const commands = {
  pillView: () => invoke<PillView | null>('pill_view'),
  pillCard: () => invoke<Card | null>('pill_card'),
  pillClicked: () => invoke<void>('pill_clicked'),
  pillHandsFree: (stop: boolean) => invoke<void>('pill_hands_free', { stop }),
  cardOpenToday: () => invoke<void>('card_open_today'),
  cardAction: () => invoke<void>('card_action'),
  cardDismiss: () => invoke<void>('card_dismiss'),
};

export interface Events {
  pill: PillView;
  'pill-hidden': null;
  card: Card;
  level: number;
}

/** Listens to one of the app's events with its payload typed. */
export function on<E extends keyof Events>(event: E, handler: (payload: Events[E]) => void): Promise<UnlistenFn> {
  return listen<Events[E]>(event, (e) => handler(e.payload));
}
