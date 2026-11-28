<script lang="ts">
  // The speech and cleanup keys, and what the recommended pair costs, worked out in Rust from
  // the config (U-11). U-12: while the walkthrough waits, a provider or key saved in Settings is
  // tested again here.
  import { onDestroy, onMount } from 'svelte';
  import { commands, on, type KeysScreen, type Role } from '../lib/ipc';
  import Key from './Key.svelte';

  let { tested }: { tested: (role: Role, ok: boolean) => void } = $props();

  let screen = $state<KeysScreen | null>(null);
  let names = $state({ speech: 'Speechmatics', cleanup: 'Groq' });
  let picked = $state(0);
  let error = $state('');
  let speech: Key;
  let cleanup: Key;

  const choice = $derived(screen?.choices[picked]);

  async function load() {
    try {
      const k = await commands.onboardingKeysScreen();
      screen = k;
      names = { speech: k.speech, cleanup: k.cleanup };
      picked = Math.min(1, k.choices.length - 1);
    } catch (e) {
      error = String(e);
    }
  }

  function pick(i: number) {
    picked = i;
    const c = screen?.choices[i];
    // Kept for the cost hints in Settings → Providers. A file written before the setting existed
    // has no line for it; the hints then use the default.
    if (c) commands.settingsSet('dictation', 'talking_minutes_per_day', c.minutes_per_day).catch(() => {});
  }

  const stop = on('providers-changed', () => {
    load();
    speech.test();
    cleanup.test();
  });
  onMount(() => {
    // The providers' names before the costs arrive.
    commands.onboardingChecks().then((p) => {
      if (!screen) names = { speech: p.speech_provider || names.speech, cleanup: p.cleanup_provider || names.cleanup };
    });
    load();
  });
  onDestroy(() => stop.then((off) => off()));
</script>

<section class="screen" data-step="keys">
  <h1>Connect your speech and text services</h1>
  <p class="lead">HyperMe uses your own keys. Nothing goes through our servers.</p>
  <div class="recommended">
    <span class="tag">Recommended</span><b id="pair-line">{names.speech} hears you, {names.cleanup} cleans up the text.</b>
    <span class="faint">The fastest pair we measured: 0.43 s from key release to text.</span>
  </div>

  <Key bind:this={speech} role="speech" name={names.speech} tested={(ok) => tested('speech', ok)} />
  <Key bind:this={cleanup} role="cleanup" name={names.cleanup} tested={(ok) => tested('cleanup', ok)}>
    <div class="hint faint">No key yet? Get a key, sign up, create an API key and paste it here.</div>
    <div class="hint faint" id="other-keys">{(screen?.other_keys ?? []).join(' ')}</div>
  </Key>

  <div class="section-title">How much do you talk a day?</div>
  <div class="seg" id="choices" role="radiogroup" aria-label="How much do you talk a day?">
    {#each screen?.choices ?? [] as c, i (c.label)}
      <button role="radio" aria-checked={i === picked} class:on={i === picked} onclick={() => pick(i)}>
        <span>{c.label}</span><small>{c.detail.split(',')[0]}</small>
      </button>
    {/each}
  </div>
  <div class="cost">
    <strong id="cost-total">{choice?.shown_as ?? ''}</strong>
    <span class="faint" id="cost-detail">
      {#if choice && screen}{screen.speech} about ${choice.speech_usd.toFixed(2)} · {screen.cleanup} about ${choice.cleanup_usd.toFixed(2)}{/if}
    </span>
  </div>
  <p class="faint" id="free-credit">{error || screen?.free_credit || ''}</p>
</section>

<style>
  .recommended { padding: var(--space-3) var(--space-4); border-radius: var(--radius-sheet); margin: calc(-1 * var(--space-2)) 0 var(--space-5); background: var(--color-accent-soft); border: 1px solid var(--color-focus-ring); }
  .tag { display: inline-block; font-size: var(--text-subheadline); font-weight: 600; color: var(--color-on-accent); background: var(--color-accent); border-radius: var(--radius-control); padding: 0 var(--space-2); margin-right: var(--space-2); vertical-align: 1px; }
  .recommended b { font-weight: 600; }
  .recommended .faint { display: block; margin-top: var(--space-1); }
  .hint { margin-top: var(--space-1); }
  .section-title { font-weight: 600; margin: var(--space-5) 0 var(--space-2); }
  .seg { display: grid; grid-template-columns: repeat(3, 1fr); gap: var(--space-1); padding: var(--space-1); border-radius: var(--radius-card); background: var(--color-fill); border: 1px solid var(--color-separator); }
  .seg button { border: 0; background: transparent; border-radius: var(--radius-control); padding: var(--space-2) var(--space-2); display: flex; flex-direction: column; align-items: center; gap: 0; }
  .seg button small { color: var(--color-label-secondary); font-size: var(--text-subheadline); font-weight: 400; }
  .seg button.on { background: var(--color-control-bg); box-shadow: var(--shadow-control); }
  .seg button:hover:not(.on) { background: transparent; }
  .cost { display: flex; align-items: baseline; gap: var(--space-3); margin-top: var(--space-3); }
  .cost strong { font: 600 var(--text-title2)/1.1 var(--font-system); }
  #free-credit { margin: var(--space-2) 0 0; }
</style>
