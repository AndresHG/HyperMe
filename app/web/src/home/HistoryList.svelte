<script lang="ts">
  // A list that keeps only the rows near the viewport in the page (AC-2). Collapsed rows and day
  // headings have fixed heights; an open row is measured. Item data stays in memory (a few
  // hundred bytes each); only elements come and go.
  import { SvelteMap } from 'svelte/reactivity';
  import type { Item } from '../lib/ipc';
  import { dayLabel } from './format';
  import HistoryRow from './HistoryRow.svelte';
  import { stateOf } from './rows.svelte';

  const ROW_H = 56;
  const DAY_H = 40;
  const OVERSCAN = 600;

  let {
    items,
    words,
    selected,
    scroller,
    onselect,
    onrun,
    onmenu,
    onfixtext,
    onsendfix,
  }: {
    items: Item[];
    words: string[];
    selected: number | null;
    /** The element that scrolls the list (Home's main). */
    scroller: HTMLElement;
    onselect: (id: number) => void;
    onrun: (id: number, action: string) => void;
    onmenu: () => void;
    onfixtext: (id: number) => void;
    onsendfix: (id: number) => void;
  } = $props();

  type Entry =
    | { key: string; day: string; top: number }
    | { key: string; item: Item; first: boolean; last: boolean; top: number };

  const tall = new SvelteMap<number, number>();
  const rows = new SvelteMap<number, HistoryRow>();
  let el = $state<HTMLDivElement>();
  // Where the viewport is, read on scroll and resize.
  let view = $state({ top: 0, height: 0, offset: 0 });

  const height = (e: Entry) =>
    'day' in e ? DAY_H : e.item.id === selected || stateOf(e.item.id).working ? (tall.get(e.item.id) ?? ROW_H * 3) : ROW_H;

  const layout = $derived.by(() => {
    const entries: Entry[] = [];
    let label: string | null = null;
    for (const item of items) {
      const day = dayLabel(item.started_ms);
      const first = day !== label;
      if (first) {
        const prev = entries[entries.length - 1];
        if (prev && 'item' in prev) prev.last = true;
        entries.push({ key: 'day:' + day, day, top: 0 });
        label = day;
      }
      entries.push({ key: 'row:' + item.id, item, first, last: false, top: 0 });
    }
    const end = entries[entries.length - 1];
    if (end && 'item' in end) end.last = true;
    let top = 0;
    for (const e of entries) {
      e.top = top;
      top += height(e);
    }
    return { entries, total: top };
  });

  const visible = $derived.by(() => {
    const from = view.top - view.offset - OVERSCAN;
    const to = view.top - view.offset + view.height + OVERSCAN;
    return layout.entries.filter((e) => e.top + height(e) >= from && e.top <= to);
  });

  /** Reads where the viewport is. */
  export function measure() {
    view = { top: scroller.scrollTop, height: scroller.clientHeight, offset: el?.offsetTop ?? 0 };
  }

  /** Scrolls so the row shows whole, and focuses it. */
  export function reveal(id: number) {
    const entry = layout.entries.find((e) => 'item' in e && e.item.id === id);
    const top = (el?.offsetTop ?? 0) + (entry?.top ?? 0);
    const h = tall.get(id) ?? ROW_H * 3;
    if (top < scroller.scrollTop) scroller.scrollTop = top - 8;
    else if (top + h > scroller.scrollTop + scroller.clientHeight) scroller.scrollTop = top + h - scroller.clientHeight + 8;
    measure();
  }

  export function row(id: number): HistoryRow | undefined {
    return rows.get(id);
  }

  $effect(() => {
    measure();
    const onScroll = () => measure();
    scroller.addEventListener('scroll', onScroll, { passive: true });
    window.addEventListener('resize', onScroll);
    return () => {
      scroller.removeEventListener('scroll', onScroll);
      window.removeEventListener('resize', onScroll);
    };
  });
</script>

<div id="list" bind:this={el} style:height={items.length ? layout.total + 'px' : ''}>
  {#each visible as entry (entry.key)}
    {#if 'day' in entry}
      <div class="day" style:top="{entry.top}px">{entry.day}</div>
    {:else}
      {@const id = entry.item.id}
      <HistoryRow
        bind:this={() => rows.get(id), (r) => (r ? rows.set(id, r) : rows.delete(id))}
        item={entry.item}
        {words}
        first={entry.first}
        last={entry.last}
        selected={id === selected}
        top={entry.top}
        measured={(h) => h && h !== tall.get(id) && tall.set(id, h)}
        onselect={() => onselect(id)}
        onrun={(action) => onrun(id, action)}
        {onmenu}
        onfixtext={() => onfixtext(id)}
        onsendfix={() => onsendfix(id)}
      />
    {/if}
  {/each}
</div>

<style>
  #list { position: relative; }
  .day { position: absolute; left: 0; right: 0; height: 40px; padding: var(--space-5) 0 0; font-size: var(--text-subheadline); font-weight: 600; letter-spacing: .06em; text-transform: uppercase; color: var(--color-label-secondary); }
</style>
