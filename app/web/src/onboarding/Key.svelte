<script lang="ts">
  // One key field. It shows its own state: testing, works with the time, or the provider's error
  // under it. Test or Enter saves a typed key in the keychain, which makes Rust send
  // `providers-changed`, and the keys screen then tests both keys again.
  import type { Snippet } from 'svelte';
  import { commands, type Role } from '../lib/ipc';
  import Status, { type Tone } from './Status.svelte';

  let { role, name, tested, children }: { role: Role; name: string; tested: (ok: boolean) => void; children?: Snippet } =
    $props();

  let value = $state('');
  let tone = $state<Tone>('');
  let text = $state('');
  let error = $state('');
  let saved = $state(false);

  function show(t: Tone, words: string, why = '') {
    tone = t;
    text = words;
    error = why;
  }

  /** Tests the key in the keychain. */
  export async function test() {
    show('busy', 'Testing…');
    try {
      const ms = await commands.keyTest(role, null, null);
      show('ok', `Works · ${ms} ms`);
      saved = true;
      tested(true);
    } catch (e) {
      // A key not typed yet is not an error: saving the other key tests both.
      if (/^No \S+ key yet$/.test(String(e))) show('', '');
      else show('bad', 'Not working', String(e));
      tested(false);
    }
  }

  async function save() {
    if (!value.trim()) return test();
    try {
      await commands.keySet(role, value, null);
      value = '';
    } catch (e) {
      show('bad', 'Not saved', String(e));
    }
  }
</script>

<div class="key">
  <div class="key-head">
    <label for="{role}-key"><span id="{role}-name">{name}</span> key</label>
    <button class="link" onclick={() => commands.onboardingGetKey(role)}>Get a key ↗</button>
  </div>
  <div class="field" class:ok={tone === 'ok'} class:bad={tone === 'bad'} id="{role}-field">
    <input
      type="password"
      id="{role}-key"
      placeholder={saved ? 'Saved in your keychain' : 'Paste your key'}
      autocomplete="off"
      spellcheck="false"
      bind:value
      onkeydown={(e) => e.key === 'Enter' && save()}
    />
    <Status id="{role}-state" {tone} {text} />
    <button onclick={save}>Test</button>
  </div>
  {#if error}<div class="error" id="{role}-error">{error}</div>{/if}
  {@render children?.()}
</div>

<style>
  .key { margin-bottom: var(--space-4); }
  .key-head { display: flex; align-items: baseline; justify-content: space-between; margin-bottom: var(--space-2); }
  .key-head label { font-weight: 600; }
  .field {
    display: flex; align-items: center; gap: var(--space-3); background: var(--color-content-bg);
    border: 1px solid var(--color-control-border); border-radius: var(--radius-card); padding: var(--space-1) var(--space-1) var(--space-1) var(--space-3);
    transition: border-color 120ms, box-shadow 120ms;
  }
  .field:focus-within { border-color: var(--color-accent); box-shadow: 0 0 0 3px var(--color-accent-soft); }
  .field.ok { border-color: color-mix(in srgb, var(--color-green-text) 55%, transparent); }
  .field.bad { border-color: color-mix(in srgb, var(--color-red) 60%, transparent); }
  .field input {
    flex: 1; min-width: 0; border: 0; outline: 0; background: transparent; color: var(--color-label);
    font: var(--text-body) var(--font-mono); padding: var(--space-2) 0;
  }
  .field input::placeholder { color: var(--color-label-secondary); font-family: var(--font-system); }
  .field button { padding: var(--space-1) var(--space-3); border-radius: var(--radius-control); }
  .error { color: var(--color-red); font-size: var(--text-callout); margin-top: var(--space-1); }
</style>
