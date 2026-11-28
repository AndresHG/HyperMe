<script lang="ts">
  // Accessibility and Input Monitoring. Checked every second while this screen is open, and at
  // once when the window comes back from System Settings. The checks never prompt, so macOS's
  // dialog never comes back by itself.
  import { onDestroy, onMount } from 'svelte';
  import { SvelteSet } from 'svelte/reactivity';
  import { commands, type Permissions, type Step } from '../lib/ipc';
  import Status from './Status.svelte';

  let { step, checked }: { step: Step; checked: (p: Permissions) => void } = $props();

  type What = 'accessibility' | 'input-monitoring';
  const rows: { what: What; glyph: string; name: string; why: string; id: string }[] = [
    { what: 'accessibility', glyph: '⌘', name: 'Accessibility', why: 'To paste your words where you type', id: 'ax-state' },
    { what: 'input-monitoring', glyph: 'fn', name: 'Input Monitoring', why: 'To notice when you hold the dictation key', id: 'im-state' },
  ];

  let perms = $state<Permissions | null>(null);
  let error = $state('');
  // Which Allow buttons have shown macOS's dialog already, and whether the user has been to
  // System Settings since.
  const prompted = new SvelteSet<What>();
  let visitedSettings = $state(false);

  const allowed = (what: What) => (what === 'accessibility' ? perms?.accessibility : perms?.input_monitoring) ?? false;
  const relaunch = $derived(visitedSettings && !!perms && !(perms.accessibility && perms.input_monitoring));

  async function check() {
    const p = await commands.onboardingChecks();
    perms = p;
    checked(p);
  }

  // The first click shows macOS's own dialog, which adds HyperMe's row and has its own "Open
  // System Settings" button; later clicks open System Settings directly.
  async function request(what: What) {
    const prompt = !prompted.has(what);
    prompted.add(what);
    visitedSettings = true;
    try {
      await commands.onboardingRequest(what, prompt);
    } catch (e) {
      error = String(e);
    }
  }

  async function relaunchApp() {
    try {
      await commands.onboardingRelaunch(step);
    } catch (e) {
      error = String(e);
    }
  }

  // Back from System Settings: check at once.
  const onFocus = () => setTimeout(check, 300);
  let poll: ReturnType<typeof setInterval>;
  onMount(() => {
    check();
    poll = setInterval(check, 1000);
    window.addEventListener('focus', onFocus);
  });
  onDestroy(() => {
    clearInterval(poll);
    window.removeEventListener('focus', onFocus);
  });
</script>

<section class="screen" data-step="permissions">
  <h1>Let HyperMe paste and hear the key</h1>
  <p class="lead" id="host-note">
    {#if error}{error}
    {:else if perms?.host_app}
      <!-- Run from a terminal (cargo run), macOS gives the permissions to that terminal, which
           must be quit and reopened before they take effect. -->
      HyperMe is running from <b>{perms.host_app}</b>, so macOS lists <b>{perms.host_app}</b>, not HyperMe. Click Allow,
      switch <b>{perms.host_app}</b> on, then quit <b>{perms.host_app}</b> (⌘Q), open it again and start HyperMe again.
    {:else}
      Click Allow on each row, then switch <b>HyperMe</b> on in the list that opens. Each row turns green by itself.
    {/if}
  </p>
  <div class="card">
    {#each rows as row (row.what)}
      <div class="perm">
        <div class="glyph" aria-hidden="true">{row.glyph}</div>
        <div class="what"><b>{row.name}</b><small>{row.why}</small></div>
        {#if perms}
          <Status id={row.id} tone={allowed(row.what) ? 'ok' : 'bad'} text={allowed(row.what) ? 'Allowed' : 'Not yet'} />
        {/if}
        {#if !allowed(row.what)}
          <button onclick={() => request(row.what)}>{prompted.has(row.what) ? 'Open Settings' : 'Allow'}</button>
        {/if}
      </div>
    {/each}
  </div>
  {#if relaunch}
    <div class="callout" id="relaunch-row">
      <span><b>Switched them on already?</b> macOS sometimes passes a permission on only after a restart.</span>
      <button id="relaunch" onclick={relaunchApp}>Relaunch HyperMe</button>
    </div>
  {/if}
</section>

<style>
  .perm { display: flex; align-items: center; gap: var(--space-4); padding: var(--space-4) var(--space-4); }
  .perm + .perm { border-top: 1px solid var(--color-separator); }
  .glyph {
    width: 34px; height: 34px; border-radius: var(--radius-card); flex: none; display: grid; place-items: center;
    background: var(--color-accent-soft); color: var(--color-accent); font: 600 var(--text-body) var(--font-system);
  }
  .what { flex: 1; min-width: 0; }
  .what b { display: block; font-weight: 600; }
  .what small { color: var(--color-label-secondary); font-size: var(--text-callout); }
  .callout {
    margin-top: var(--space-4); padding: var(--space-3) var(--space-4); border-radius: var(--radius-card); background: var(--color-fill);
    color: var(--color-label-secondary); font-size: var(--text-callout); display: flex; align-items: center; justify-content: space-between; gap: var(--space-3);
  }
  .callout b { color: var(--color-label); font-weight: 600; }
  .callout button { white-space: nowrap; }
</style>
