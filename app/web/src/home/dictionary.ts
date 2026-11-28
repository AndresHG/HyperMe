// The Dictionary page's rows (spec 93): entries ({written, heard[]}) to rows ({word, wrong}) and
// back. Each misspelling is its own row ("zen trix → Zentrix"); a word with no misspelling is a
// row of its own.
import type { DictionaryEntry } from '../lib/ipc';

export interface Row {
  word: string;
  wrong: string;
}

// The matcher compares lower case without accents (ñ stays), so the page groups rows the same.
export const key = (s: string) =>
  s.trim().toLowerCase().replace(/[áàéèíìóòúùü]/g, (c) => 'aaeeiioouuu'['áàéèíìóòúùü'.indexOf(c)] ?? c);

export const toRows = (entries: DictionaryEntry[]): Row[] =>
  entries.flatMap((e) => (e.heard.length ? e.heard.map((h) => ({ word: e.written, wrong: h })) : [{ word: e.written, wrong: '' }]));

export function toEntries(rows: Row[]): DictionaryEntry[] {
  const entries: DictionaryEntry[] = [];
  for (const r of rows) {
    let entry = entries.find((e) => key(e.written) === key(r.word));
    if (!entry) {
      entry = { written: r.word, heard: [] };
      entries.push(entry);
    }
    if (r.wrong && !entry.heard.some((h) => key(h) === key(r.wrong))) entry.heard.push(r.wrong);
  }
  return entries;
}
