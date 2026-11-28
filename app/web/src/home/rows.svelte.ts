// Per-row state that outlives the row and the History page: the step in progress, a notice, a
// retry, the trace once read. Item data itself is never kept here (spec 98, R-24).
import type { Trace } from '../lib/ipc';

export type Mode =
  | { kind: 'confirm-delete' }
  | { kind: 'fix-select' }
  | { kind: 'fix-type'; heard: string; wanted: string }
  | { kind: 'fix-confirm'; heard: string; wanted: string }
  // Spec 267: Correct edits the whole text; the first save asks before keeping anything (R-6).
  | { kind: 'correct'; text: string }
  | { kind: 'correct-agree'; text: string };

export interface RowState {
  mode: Mode | null;
  say: { text: string; bad: boolean } | null;
  working: boolean;
  /** undefined: not read yet; null: being read; false: none. */
  trace: Trace | null | false | undefined;
}

// A plain map of reactive states: making one while a row draws changes nothing that is drawn.
// eslint-disable-next-line svelte/prefer-svelte-reactivity -- deliberately not reactive, see above
const rows = new Map<number, RowState>();

export function stateOf(id: number): RowState {
  const known = rows.get(id);
  if (known) return known;
  const state = $state<RowState>({ mode: null, say: null, working: false, trace: undefined });
  rows.set(id, state);
  return state;
}

export function forget(id: number) {
  rows.delete(id);
}

/** A notice under the row; a good one goes after 3 s. */
export function say(id: number, text: string | null, bad = false) {
  const state = stateOf(id);
  const mine = text === null ? null : { text, bad };
  state.say = mine;
  if (mine && !bad) {
    setTimeout(() => {
      if (state.say === mine) state.say = null;
    }, 3000);
  }
}
