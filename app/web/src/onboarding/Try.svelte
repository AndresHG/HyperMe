<script lang="ts">
  // A first dictation into the box, with its time from key release.
  import { onDestroy, onMount } from 'svelte';
  import { commands, on } from '../lib/ipc';
  import Status, { type Tone } from './Status.svelte';

  let { pasted }: { pasted: () => void } = $props();

  let box: HTMLTextAreaElement;
  let tone = $state<Tone>('');
  let text = $state('');

  const stop = on('dictation-pasted', (ms) => {
    tone = 'ok';
    text = `Pasted in ${(ms / 1000).toFixed(2)} s`;
    pasted();
  });
  onMount(async () => {
    try {
      await commands.onboardingTry();
      box.focus();
    } catch (e) {
      tone = 'bad';
      text = String(e);
    }
  });
  onDestroy(() => stop.then((off) => off()));
</script>

<section class="screen" data-step="try">
  <h1>Try it</h1>
  <p class="lead">Click in the box, hold <kbd>fn</kbd> and say anything. Let go when you are done.</p>
  <textarea bind:this={box} id="try-box" placeholder="Your words appear here"></textarea>
  <div class="result"><Status id="try-state" large {tone} {text} /></div>
</section>

<style>
  textarea {
    width: 100%; height: 160px; padding: var(--space-4); border-radius: var(--radius-sheet); resize: none;
    background: var(--color-content-bg); color: var(--color-label); border: 1px solid var(--color-control-border);
    font: var(--text-title3)/1.5 var(--font-system); transition: border-color 120ms, box-shadow 120ms;
  }
  textarea:focus { outline: 0; border-color: var(--color-accent); box-shadow: 0 0 0 3px var(--color-accent-soft); }
  kbd { font: 600 var(--text-subheadline) var(--font-system); padding: 0 var(--space-2); border-radius: var(--radius-small); background: var(--color-content-bg); border: 1px solid var(--color-control-border); border-bottom-width: 2px; color: var(--color-label); }
  .result { margin-top: var(--space-4); min-height: 20px; }
</style>
