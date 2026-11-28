// Dates, durations and search marks for Home. The page's copy is English, so dates and numbers
// are too, until the app is translated.
export const LOCALE = 'en-GB';
export const DAY_MS = 24 * 60 * 60 * 1000;
export const timeFormat = new Intl.DateTimeFormat(LOCALE, { hour: '2-digit', minute: '2-digit' });
export const dayFormat = new Intl.DateTimeFormat(LOCALE, { weekday: 'long', day: 'numeric', month: 'long' });

export function startOfDay(ms: number): number {
  const d = new Date(ms);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

/** Today, Yesterday, or the date. */
export function dayLabel(ms: number): string {
  const today = startOfDay(Date.now());
  const day = startOfDay(ms);
  if (day === today) return 'Today';
  if (day === startOfDay(today - DAY_MS / 2)) return 'Yesterday';
  return dayFormat.format(new Date(ms));
}

/** 45 s, 12 min, 1 h 5 min. */
export function duration(seconds: number): string {
  if (seconds < 60) return seconds + ' s';
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return minutes + ' min';
  return Math.floor(minutes / 60) + ' h ' + (minutes % 60) + ' min';
}

/** Morning, afternoon or evening. */
export function greeting(hour = new Date().getHours()): string {
  return hour < 5 ? 'Good evening' : hour < 12 ? 'Good morning' : hour < 19 ? 'Good afternoon' : 'Good evening';
}

// Case and accents fold as the search does (R-12), keeping each character's place, so the
// matches can be marked in the original text.
const foldChar = (c: string) => c.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();

/** The search's words, folded, each once. */
export function queryWords(query: string): string[] {
  const q = query.trim();
  return q ? Array.from(new Set(q.split(/\s+/).map((w) => Array.from(w).map(foldChar).join('')))) : [];
}

/** `text` cut into pieces, each marked where it matches one of `words`. */
export function marks(text: string, words: string[]): { text: string; marked: boolean }[] {
  if (words.length === 0) return [{ text, marked: false }];
  const chars = Array.from(text);
  let folded = '';
  const start: number[] = [];
  chars.forEach((c, i) => {
    const f = foldChar(c);
    for (let k = 0; k < f.length; k++) start.push(i);
    folded += f;
  });
  const marked = new Array<boolean>(chars.length).fill(false);
  for (const w of words) {
    let at = folded.indexOf(w);
    while (at !== -1) {
      for (let k = at; k < at + w.length; k++) marked[start[k] ?? 0] = true;
      at = folded.indexOf(w, at + w.length);
    }
  }
  const pieces: { text: string; marked: boolean }[] = [];
  let i = 0;
  while (i < chars.length) {
    let j = i;
    while (j < chars.length && marked[j] === marked[i]) j++;
    pieces.push({ text: chars.slice(i, j).join(''), marked: marked[i] ?? false });
    i = j;
  }
  return pieces;
}
