<script lang="ts">
  // The microphone check: 24 bars shaped like a hill, driven by the live level. The level comes
  // from Rust (`onboarding-level`) while this screen is open.
  import { onDestroy, onMount } from 'svelte';
  import { commands, on } from '../lib/ipc';
  import Status, { type Tone } from './Status.svelte';

  let { heard }: { heard: () => void } = $props();

  const BARS = 24;
  const shape = Array.from({ length: BARS }, (_, i) => 0.35 + 0.65 * Math.sin((Math.PI * (i + 0.5)) / BARS));
  let heights = $state(new Array<number>(BARS).fill(6));
  let live = $state(false);
  let tone = $state<Tone>('busy');
  let text = $state('Listening…');
  let working = false;

  const stop = on('onboarding-level', ([raw, voice]) => {
    const level = raw ?? 0;
    live = level > 0.05;
    heights = shape.map((s) => Math.max(6, Math.round(60 * level * s * (0.75 + 0.25 * Math.random()))));
    if (voice && !working) {
      working = true;
      tone = 'ok';
      text = 'Working';
      heard();
    }
  });

  onMount(() => {
    commands.onboardingMic(true).catch((e) => {
      tone = 'bad';
      text = String(e);
    });
  });
  onDestroy(() => {
    stop.then((off) => off());
    commands.onboardingMic(false);
  });
</script>

<section class="screen" data-step="microphone">
  <h1>Check your microphone</h1>
  <p class="lead">Say a few words. If macOS asks whether HyperMe can use the microphone, allow it.</p>
  <div class="card meter-card">
    <div class="bars" class:live aria-hidden="true">{#each heights as h, i (i)}<i style:height="{h}px"></i>{/each}</div>
    <Status id="mic-state" {tone} {text} />
  </div>
</section>

<style>
  .meter-card { padding: var(--space-6) var(--space-6) var(--space-5); display: flex; flex-direction: column; gap: var(--space-4); align-items: center; }
  .bars { display: flex; align-items: center; gap: var(--space-1); height: 60px; }
  .bars i { display: block; width: 5px; height: 6px; border-radius: var(--radius-small); background: var(--color-control-border); transition: height 60ms linear, background 160ms; }
  .bars.live i { background: var(--color-accent); }
</style>
