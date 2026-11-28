<script lang="ts">
  // The pill (docs/UX.md, "The pill" and "The voice while you talk", U-13 to U-16). Rust sends
  // each state as the `pill` event and sizes the panel; this page draws it. Nothing runs while
  // the pill is hidden or at rest.
  import { onMount } from 'svelte';
  import { commands, on, type Card, type PillView } from '../lib/ipc';
  import CardView from './Card.svelte';
  import { Voice, type Motion } from './voice';

  let kind = $state('hidden');
  let label = $state('');
  let card = $state<Card | null>(null);
  let canvas: HTMLCanvasElement;
  let voice: Voice;

  // Recording, held or hands-free: the bars follow the voice.
  const voiced = (k: string) => k === 'recording' || k === 'locked';
  const motionOf = (k: string): Motion => (voiced(k) ? 'voice' : k === 'working' ? 'working' : 'still');
  const moving = $derived(motionOf(kind) !== 'still');
  const muted = $derived(kind === 'quiet' || kind === 'notice');

  function render(view: PillView) {
    card = null;
    if (view.kind === 'card') {
      commands.pillCard().then((c) => {
        if (c) showCard(c);
      });
      return;
    }
    kind = view.kind;
    label = view.label;
    voice.set(motionOf(kind));
  }

  function hide() {
    card = null;
    kind = 'hidden';
    voice.set('still');
  }

  function showCard(view: Card) {
    hide();
    kind = 'card';
    card = view;
  }

  function handsFree(event: MouseEvent, stop: boolean) {
    // #167 AC-4: the click stays off the pill itself.
    event.stopPropagation();
    commands.pillHandsFree(stop);
  }

  onMount(() => {
    voice = new Voice(canvas, window.matchMedia('(prefers-reduced-motion: reduce)'));
    // Ready for the next key-down before its event arrives, but still.
    voice.set('voice');
    voice.set('still');
    // A view sent before these listeners exist is lost (the bead is sent at launch), so once
    // they are in place the page asks for the current one, unless an event has already come.
    let heard = false;
    Promise.all([
      on('pill', (view) => {
        heard = true;
        render(view);
      }),
      on('pill-hidden', () => {
        heard = true;
        hide();
      }),
      on('card', (view) => {
        heard = true;
        showCard(view);
      }),
      on('level', (level) => voice.level(level ?? 0)),
    ])
      .then(() => commands.pillView())
      .then((view) => {
        if (!heard && view) render(view);
      });
  });
</script>

<!-- The panel never takes focus (it would steal it from the app being dictated into), so the
     pill answers clicks only. -->
<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
<div id="pill" class="pill {kind}" role="status" aria-live="polite" onclick={() => commands.pillClicked()}>
  <!-- Each state has an icon and a word, never colour alone (docs/UX.md). -->
  <span class="icon">{#if kind === 'recording'}<span class="dot"></span>{:else if kind === 'notice'}<span class="muted">🔒</span>{:else if kind === 'error'}<span class="alarm">!</span>{:else if kind === 'retry'}<span class="alarm">↻</span>{/if}</span>
  <canvas bind:this={canvas} class:on={moving} aria-hidden="true"></canvas>
  <span class="label" class:muted>{label}</span>
  <button class="hf cancel" aria-label="Cancel dictation" title="Cancel (Esc)" onclick={(e) => handsFree(e, false)}>✕</button>
  <button class="hf stop" aria-label="Done, paste" title="Done, paste" onclick={(e) => handsFree(e, true)}>✓</button>
</div>
<CardView {card} />

<style>
  /* Tokens and states from docs/UX.md, "Visual language", "The pill" and "The voice while you
     talk" (U-14, U-15). */
  .pill {
    position: relative;
    width: 100%;
    height: 100%;
    border-radius: var(--radius-capsule);
    background: var(--color-pill-bg);
    box-shadow: inset 0 0 0 1px var(--color-pill-border);
    color: var(--color-pill-label);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0 var(--space-8);
    font: 500 var(--text-body) var(--font-system);
    white-space: nowrap;
    /* The one spring from the bead's centre for every size change (docs/UX.md, 180 ms). */
    transition: width var(--duration) var(--easing), height var(--duration) var(--easing);
  }
  .pill.error, .pill.retry { box-shadow: inset 0 0 0 1.5px var(--color-recording); }
  /* The bead at rest (U-13, B: glass): a small dark glass bead with a soft highlight. Still:
     no animation runs while idle. */
  .pill.idle {
    width: 14px;
    height: 14px;
    padding: 0;
    background: var(--pill-bead);
    box-shadow: var(--shadow-bead);
  }
  .pill.idle::after {
    content: '';
    position: absolute;
    left: 3px;
    top: 2.5px;
    width: 4px;
    height: 4px;
    border-radius: var(--radius-capsule);
    background: var(--pill-bead-highlight);
  }
  .pill.idle > * { display: none !important; }
  /* Until the first view arrives, and whenever the panel is hidden or shows a card, nothing is
     drawn. */
  .pill.hidden, .pill.card { display: none; }
  /* The state's mark sits at the left edge, so the label and the voice stay in the exact centre. */
  .icon { position: absolute; left: 14px; top: 0; bottom: 0; display: flex; align-items: center; }
  .icon:empty { display: none; }
  .dot { width: 8px; height: 8px; border-radius: var(--radius-capsule); background: var(--color-recording); }
  .alarm { color: var(--color-recording); }
  /* Hands-free (#167): ✕ cancels at the left, ✓ stops and pastes at the right. Shown only in
     the locked state. */
  .hf {
    position: absolute; top: 0; bottom: 0; margin: auto 0; width: 20px; height: 20px;
    border: 0; padding: 0; border-radius: var(--radius-capsule); display: none; align-items: center;
    justify-content: center; background: var(--color-pill-hover); color: var(--color-pill-label-secondary);
    font: 600 var(--text-subheadline) var(--font-system);
  }
  .hf:hover { color: var(--color-pill-label); background: var(--color-pill-pressed); }
  .hf.cancel { left: 8px; }
  /* Green check, not a red stop: it accepts the dictation and pastes it (owner, 27 November). */
  .hf.stop { right: 8px; background: var(--color-done-soft); color: var(--color-done); font-size: var(--text-callout); }
  .hf.stop:hover { background: var(--color-done-soft-strong); color: var(--color-done); }
  .pill.locked .hf { display: flex; }
  canvas { flex: none; display: none; }
  canvas.on { display: block; }
  @media (prefers-reduced-motion: reduce) {
    /* Reduce motion: no spring, a short cross-fade instead. */
    .pill { transition: opacity 120ms linear; }
  }
  .label { overflow: hidden; text-overflow: ellipsis; }
  .label:empty { display: none; }
  .muted { color: var(--color-pill-label-secondary); }
</style>
