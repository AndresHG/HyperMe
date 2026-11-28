<script lang="ts">
  // The walkthrough's frame: one screen at a time, the dots, and the one primary button. What
  // each screen has seen work is kept in `checks`; Rust decides whether the button lights up
  // (onboarding.rs, `can_continue`).
  import { commands, type Checks, type Step } from '../lib/ipc';
  import Keys from './Keys.svelte';
  import Microphone from './Microphone.svelte';
  import Permissions from './Permissions.svelte';
  import Try from './Try.svelte';
  import Welcome from './Welcome.svelte';

  const steps: Step[] = ['welcome', 'microphone', 'permissions', 'keys', 'try'];
  const labels: Partial<Record<Step, string>> = { welcome: 'Get started', try: 'Done' };

  // After "Relaunch HyperMe" the window opens on the screen it was on.
  let index = $state(Math.max(0, steps.indexOf(location.hash.slice(1) as Step)));
  const step = $derived(steps[index] ?? 'welcome');
  const checks = $state<Checks>({
    microphone: false,
    accessibility: false,
    input_monitoring: false,
    speech_key: false,
    cleanup_key: false,
    tried: false,
  });
  let canContinue = $state(false);

  // Asks Rust again whenever the step or a check changes; a late answer for an earlier state is
  // dropped.
  let asked = 0;
  $effect(() => {
    const ask = ++asked;
    commands.onboardingCanContinue(step, $state.snapshot(checks)).then((ok) => {
      if (ask === asked) canContinue = ok;
    });
  });

  function next() {
    if (index === steps.length - 1) commands.onboardingDone();
    else index += 1;
  }
</script>

<div id="titlebar" data-tauri-drag-region></div>
<main>
  {#if step === 'welcome'}
    <Welcome />
  {:else if step === 'microphone'}
    <Microphone heard={() => (checks.microphone = true)} />
  {:else if step === 'permissions'}
    <Permissions
      {step}
      checked={(p) => {
        checks.accessibility = p.accessibility;
        checks.input_monitoring = p.input_monitoring;
      }}
    />
  {:else if step === 'keys'}
    <Keys tested={(role, ok) => (checks[role === 'speech' ? 'speech_key' : 'cleanup_key'] = ok)} />
  {:else}
    <Try pasted={() => (checks.tried = true)} />
  {/if}
</main>
<footer>
  <div class="dots" id="dots">
    {#each steps as s, i (s)}<span class:on={i === index} class:done={i < index}></span>{/each}
  </div>
  {#if step === 'keys'}
    <button class="quiet" id="other-providers" onclick={() => commands.onboardingOtherProviders()}>Use other providers</button>
  {/if}
  <button class="primary" id="next" disabled={!canContinue} onclick={next}>{labels[step] ?? 'Continue'}</button>
</footer>

<style>
  footer { display: flex; align-items: center; gap: var(--space-4); padding: var(--space-4) var(--space-8) var(--space-5); }
  .dots { display: flex; gap: var(--space-2); flex: 1; }
  .dots span { width: 6px; height: 6px; border-radius: var(--radius-small); background: var(--color-control-border); transition: width 200ms, background 200ms; }
  .dots span.done { background: var(--color-focus-ring); }
  .dots span.on { width: 18px; background: var(--color-accent); }
</style>
