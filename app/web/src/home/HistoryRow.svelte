<script lang="ts">
  // One dictation in History (R-8, R-11, R-13 to R-18): the time, the text with the search's
  // marks, the app, and its tools on hover. Open (selected, or a retry in progress) it shows the
  // bar under it: its notice or the step in progress, and, selected, the trace's stage bars
  // (spec 200, R-10, R-11).
  import { commands, type Item } from '../lib/ipc';
  import { marks, timeFormat } from './format';
  import Icon from './Icon.svelte';
  import { stateOf } from './rows.svelte';
  import { openSettings } from './settings.svelte';

  let {
    item,
    words,
    first,
    last,
    selected,
    top,
    measured,
    onselect,
    onrun,
    onmenu,
    onfixtext,
    onsendfix,
  }: {
    item: Item;
    words: string[];
    first: boolean;
    last: boolean;
    selected: boolean;
    top: number;
    /** The open row's height, once drawn. */
    measured: (height: number) => void;
    onselect: () => void;
    onrun: (action: string) => void;
    onmenu: () => void;
    /** A selection in the text, maybe the word to fix (R-17). */
    onfixtext: () => void;
    onsendfix: () => void;
  } = $props();

  const row = $derived(stateOf(item.id));
  const open = $derived(selected || row.working);
  const failed = $derived(item.state === 'failed');
  const has = (id: string) => item.actions.some((a) => a.id === id);
  let node = $state<HTMLDivElement>();
  let fixInput = $state<HTMLInputElement>();

  // The open row is measured, so the rows under it move down.
  $effect(() => {
    if (!open || !node) return;
    const el = node;
    const observer = new ResizeObserver(() => measured(el.offsetHeight));
    observer.observe(el);
    return () => observer.disconnect();
  });

  // The trace is read once per item, when it first opens; nothing shows for an item without one.
  $effect(() => {
    if (!selected || row.trace !== undefined) return;
    const s = row;
    s.trace = null;
    commands
      .historyTrace(item.id)
      .then((t) => (s.trace = t ?? false))
      .catch(() => (s.trace = false));
  });

  $effect(() => {
    if (row.mode?.kind === 'fix-type') fixInput?.focus();
  });

  const STAGE_NAMES: Record<string, string> = {
    mic_start: 'Microphone start',
    speech: 'Speech',
    dictionary: 'Dictionary',
    cleanup: 'Cleanup',
    insert: 'Paste',
    history_write: 'History write',
  };
  const trace = $derived(row.trace || null);
  const end = $derived(trace ? Math.max(...trace.bars.map((b) => b.start_ms + b.duration_ms)) || 1 : 1);
  const via = $derived(
    trace
      ? [
          trace.speech && 'Speech: ' + trace.speech.join(' · '),
          trace.cleanup && 'Cleanup: ' + trace.cleanup.join(' · '),
          'Outcome: ' + trace.outcome.replaceAll('_', ' '),
        ]
          .filter(Boolean)
          .join('  ·  ')
      : '',
  );

  /** Focuses the row, for the arrow keys. */
  export function focus() {
    node?.focus({ preventScroll: true });
  }
  /** Where the row is on screen, for the menu. */
  export function box() {
    return node?.getBoundingClientRect();
  }
</script>

<!-- The arrow keys move the selection (History's keys); a click selects. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  bind:this={node}
  class="row"
  class:failed
  class:first
  class:last
  class:sel={selected}
  class:working={row.working}
  class:open
  data-id={item.id}
  tabindex="-1"
  style:top="{top}px"
  onmousedown={(e) => {
    if (!(e.target as Element).closest('.bar, .tools')) onselect();
  }}
>
  <time>{timeFormat.format(new Date(item.started_ms))}</time>
  <div class="body">
    <span class="text" onmouseup={onfixtext} ondblclick={() => setTimeout(onfixtext, 0)}>
      {#if failed}{item.message || 'Failed'}{:else}{#each marks(item.text || '', words) as piece, i (i)}{#if piece.marked}<mark
            >{piece.text}</mark
          >{:else}{piece.text}{/if}{/each}{/if}
    </span>
    <span class="app">{item.app || ''}</span>
  </div>
  <!-- The tools on hover, as in Wispr Flow: Copy, Paste again and the rest under More (⌘K); a
       failed dictation shows Retry instead (R-13, R-16). -->
  <div class="tools">
    {#if failed}
      {#if has('retry')}<button class="retry" onclick={() => onrun('retry')}>Retry</button>{/if}
    {:else}
      {#if has('copy')}<button class="icon" title="Copy" aria-label="Copy" onclick={() => onrun('copy')}><Icon name="copy" size={14} /></button>{/if}
      {#if has('paste')}<button class="icon" title="Paste again" aria-label="Paste again" onclick={() => onrun('paste')}
          ><Icon name="paste" size={14} /></button
        >{/if}
    {/if}
    <button
      class="icon"
      title="More"
      aria-label="More actions"
      onclick={() => {
        onselect();
        onmenu();
      }}><Icon name="more" size={14} /></button
    >
  </div>
  {#if open}
    {#if row.mode || row.working || failed || row.say}
    <div class="bar">
      {#if row.mode?.kind === 'confirm-delete'}
        <span class="ask">Delete this dictation and its audio? <kbd>↵</kbd> Delete · <kbd>Esc</kbd> Keep</span>
      {:else if row.mode?.kind === 'fix-select'}
        <span class="say">Select the wrong word in the text, or double-click it. <kbd>Esc</kbd> Cancel</span>
      {:else if row.mode?.kind === 'fix-type'}
        {@const mode = row.mode}
        <span class="say">“{mode.heard}” should be: </span>
        <input
          bind:this={fixInput}
          bind:value={mode.wanted}
          placeholder="Should be"
          aria-label="What “{mode.heard}” should be"
          onkeydown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              e.stopPropagation();
              onsendfix();
            }
            if (e.key === 'Escape') {
              e.preventDefault();
              e.stopPropagation();
              row.mode = null;
            }
          }}
        />
        <span class="say"> <kbd>↵</kbd></span>
      {:else if row.mode?.kind === 'fix-confirm'}
        <span class="ask warn">“{row.mode.heard}” is a common word and will be replaced everywhere. <kbd>↵</kbd> Replace anyway · <kbd>Esc</kbd> Cancel</span>
      {:else if row.working}
        <span class="say">Working…</span>
      {:else}
        <!-- Spec 200, R-12: a failed item can be reported, its trace leading the issue. -->
        {#if failed}<button onclick={() => openSettings('diagnostics/report/' + item.id)}>Report a problem</button>{/if}
        {#if row.say}<span class="say" class:bad={row.say.bad}>{row.say.text}</span>{/if}
      {/if}
    </div>
    {/if}
    {#if selected && trace && trace.bars.length}
      <div class="stages">
        {#each trace.bars as b, i (i)}
          <span>{STAGE_NAMES[b.stage] || b.stage}</span>
          <div class="track"><div class="fill" style:left="{(100 * b.start_ms) / end}%" style:width="{(100 * b.duration_ms) / end}%"></div></div>
          <span class="ms">{Math.round(b.duration_ms)} ms</span>
        {/each}
        <span class="via">{via}</span>
      </div>
    {/if}
  {/if}
</div>

<style>
  .row { position: absolute; left: 0; right: 0; display: grid; grid-template-columns: 56px minmax(0, 1fr) auto; column-gap: var(--space-3); align-items: start; padding: var(--space-3); height: 56px; overflow: hidden; border-radius: var(--radius-control); cursor: default; outline: none; }
  .row.open { height: auto; }
  .row:hover { background: var(--color-hover); }
  time { color: var(--color-label-secondary); font-variant-numeric: tabular-nums; font-size: var(--text-callout); padding-top: 0; }
  .body { display: flex; flex-direction: column; gap: var(--space-1); min-width: 0; }
  .text { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; line-height: 1.45; -webkit-user-select: text; user-select: text; }
  .app { font-size: var(--text-subheadline); color: var(--color-label-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .row.failed .text { color: var(--color-orange-text); }
  .tools { display: flex; gap: var(--space-1); visibility: hidden; }
  .row:hover .tools, .row.sel .tools, .row.failed .tools { visibility: visible; }
  .icon { width: 28px; height: 28px; border-radius: var(--radius-control); border: 1px solid var(--color-separator); background: var(--color-content-bg); display: flex; align-items: center; justify-content: center; padding: 0; }
  .icon:hover { border-color: var(--color-accent); }
  .retry { padding: var(--space-1) var(--space-3); border-radius: var(--radius-control); border: 1px solid var(--color-raw); background: transparent; color: var(--color-orange-text); font-size: var(--text-callout); font-weight: 600; }
  .row.sel { background: var(--color-accent-soft); }
  .row.sel .text { white-space: pre-wrap; overflow: visible; }
  .bar { grid-column: 2 / -1; display: flex; align-items: center; gap: var(--space-2); flex-wrap: wrap; padding-top: var(--space-2); font-size: var(--text-callout); }
  .bar button { font-size: var(--text-callout); padding: var(--space-1) var(--space-2); border-radius: var(--radius-control); border: 1px solid var(--color-separator); background: var(--color-content-bg); }
  .bar button:hover { border-color: var(--color-accent); }
  .say { color: var(--color-label-secondary); }
  .stages { grid-column: 1 / -1; display: grid; grid-template-columns: 110px 1fr 64px; gap: var(--space-1) var(--space-2); align-items: center; padding-top: var(--space-2); font-size: var(--text-subheadline); color: var(--color-label-secondary); }
  .track { position: relative; height: 8px; }
  .fill { position: absolute; top: 0; height: 8px; min-width: 2px; border-radius: var(--radius-small); background: var(--color-accent); opacity: 0.7; }
  .ms { text-align: right; font-variant-numeric: tabular-nums; }
  .via { grid-column: 1 / -1; }
  .say.bad { color: var(--color-red); }
  .warn { color: var(--color-orange-text); }
  .ask { color: var(--color-label); }
  .ask.warn { color: var(--color-orange-text); }
  kbd { font: var(--text-subheadline) var(--font-system); color: var(--color-label-secondary); padding: 0 var(--space-1); }
  input { font: inherit; font-size: var(--text-callout); padding: var(--space-1) var(--space-2); border-radius: var(--radius-control); border: 1px solid var(--color-accent); background: var(--color-content-bg); color: inherit; width: 200px; }
  .row.working .text { color: var(--color-label-secondary); }
  mark { background: color-mix(in srgb, var(--color-accent) 28%, transparent); color: inherit; border-radius: var(--radius-small); padding: 0 0; }
</style>
