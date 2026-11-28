<script lang="ts">
  // Connect a preset (V0.1 redesign): a two-step dialog, the main button always bottom right.
  // Step 1 is the user's own OAuth client; step 2 the account's label and what Hyper may use. The
  // label is a choice, so it can never look filled in (#216). Sign in again keeps the label.
  import { commands, type SignInResult } from '../../lib/ipc';
  import { noMark, testing, useSettings, type Mark } from '../state.svelte';

  let {
    connector,
    label: known = '',
    features: wanted = null,
    onclose,
  }: { connector: string; label?: string; features?: string[] | null; onclose: () => void } = $props();
  const s = useSettings();

  const PRESETS = ['personal', 'work'];
  const LABELS: [string, string][] = [
    ['personal', 'Personal'],
    ['work', 'Work'],
    ['other', 'Other…'],
  ];
  // The preset, kept as it was when the dialog opened; a saved client refreshes it.
  let preset = $state(s.connectors?.sheets.find((x) => x.connector === connector));
  // svelte-ignore state_referenced_locally
  let step = $state(preset?.client_saved ? 2 : 1);
  // svelte-ignore state_referenced_locally
  let choice = $state<string | null>(PRESETS.includes(known) ? known : known ? 'other' : null);
  // svelte-ignore state_referenced_locally
  let other = $state(choice === 'other' ? known : '');
  // svelte-ignore state_referenced_locally
  const features = $state(Object.fromEntries((preset?.features ?? []).map((f) => [f.id, wanted ? wanted.includes(f.id) : f.on_by_default])));
  // svelte-ignore state_referenced_locally
  let clientId = $state(preset?.client_id ?? '');
  let secret = $state('');
  // svelte-ignore state_referenced_locally
  let clientState = $state<Mark>(preset?.client_saved ? { text: 'Saved', tone: 'ok' } : noMark);
  let wait = $state<Mark>(noMark);
  let waiting = $state(false);
  let inTesting = $state(false);
  let done = $state<SignInResult | null>(null);
  let otherField = $state<HTMLInputElement>();

  const word = $derived(choice === 'other' ? other.trim() : (choice ?? ''));

  async function saveClient() {
    if (!preset) return;
    clientState = testing;
    try {
      const ms = await commands.connectorsSaveClient(preset.connector, clientId, secret);
      secret = '';
      clientState = { text: `Works, ${ms} ms`, tone: 'ok' };
      await s.refreshConnectors();
      preset = s.connectors?.sheets.find((x) => x.connector === connector) ?? preset;
      step = 2;
    } catch (e) {
      clientState = { text: String(e), tone: 'bad' };
    }
  }

  async function copyCommands() {
    if (!preset) return;
    await navigator.clipboard.writeText(preset.commands.join('\n'));
    clientState = { text: 'gcloud commands copied; the OAuth client itself is made on the setup page', tone: clientState.tone };
  }

  async function signIn() {
    if (!preset) return;
    wait = { text: 'Finish in your browser…', tone: 'warn' };
    waiting = true;
    try {
      const picked = Object.keys(features).filter((id) => features[id]);
      done = await commands.connectorsSignIn(preset.connector, word, picked);
      await s.refreshConnectors();
      wait = noMark;
    } catch (e) {
      const err = e as { message?: string; testing?: boolean };
      wait = { text: err?.message || String(e), tone: 'bad' };
      inTesting = !!err?.testing;
    } finally {
      waiting = false;
    }
  }

  function pickLabel(value: string) {
    choice = value;
    if (value === 'other') queueMicrotask(() => otherField?.focus());
  }
</script>

{#if preset}
  <div id="connectors-oauth">
    <div class="dialog" role="dialog" aria-labelledby="oauth-title">
      <header class="dialog-head">
        <span class="glyph" aria-hidden="true">{preset.name.slice(0, 1)}</span>
        <h1 id="oauth-title">Connect {preset.name}</h1>
        <span class="step-count">Step {step} of 2</span>
      </header>
      <ol class="steps" aria-label="Steps">
        <li class={step === 2 ? 'done' : 'now'}><span class="n">{step === 2 ? '✓' : '1'}</span>Your client</li>
        <li class={step === 2 ? 'now' : ''}><span class="n">2</span>Account and access</li>
      </ol>
      <div class="dialog-body">
        <div class:hidden={step === 2}>
          <p class="lead">
            HyperMe signs in with your own OAuth client, so nothing passes through us. <a
              class="link"
              href="#client"
              onclick={(e) => (e.preventDefault(), commands.connectorsOpenUrl(preset!.connector, 'client').catch(s.fail))}>How to create it</a
            >
          </p>
          <label class="field"
            ><span>Client ID <small class="state {clientState.tone}">{clientState.text}</small></span><input
              type="text"
              autocomplete="off"
              placeholder="Paste the client ID"
              bind:value={clientId}
            /></label
          >
          <label class="field"
            ><span>Client secret <small>Kept in the macOS keychain.</small></span><input
              type="password"
              autocomplete="off"
              placeholder="Paste the client secret"
              bind:value={secret}
            /></label
          >
          <p class="lead small">
            <a class="link" href="#gcloud" onclick={(e) => (e.preventDefault(), copyCommands())}>Prefer the command line? Copy the gcloud steps</a>
          </p>
        </div>
        <div class:hidden={step !== 2 || !!done}>
          <fieldset class="choice">
            <legend>What do you call this account?</legend>
            <div class="chips">
              {#each LABELS as [value, title] (value)}
                <label
                  ><input
                    type="radio"
                    name="oauth-label-choice"
                    {value}
                    checked={choice === value}
                    disabled={!!known}
                    onchange={() => pickLabel(value)}
                  />{title}</label
                >
              {/each}
            </div>
            <input
              type="text"
              class:hidden={choice !== 'other'}
              aria-label="Account name, one word"
              placeholder="One word, such as school"
              disabled={!!known}
              bind:value={other}
              bind:this={otherField}
            />
            <small class="say">{word ? `You’ll say it: “Hyper, what’s on my ${word} calendar?”` : ''}</small>
          </fieldset>
          <fieldset class="choice">
            <legend>What can Hyper use?</legend>
            <div class="checks">
              {#each preset.features as f (f.id)}
                <label data-words="scope {f.label}"><input type="checkbox" class="ui-checkbox" bind:checked={features[f.id]} /><span>{f.label}</span></label>
              {/each}
            </div>
          </fieldset>
        </div>
        <div class:hidden={!inTesting}>
          <p class="bad">Your project is in Testing, so you would be signed out every 7 days.</p>
          <p class="lead">Set its publishing status to In production (no review is needed for your own account), then sign in again.</p>
          <p><button onclick={() => commands.connectorsOpenUrl(preset!.connector, 'production').catch(s.fail)}>Open the setting</button></p>
        </div>
        {#if done}
          <div>
            <p class="ok">Signed in as {done.account}.</p>
            {#each done.checks as c, i (i)}<p class="lead">{c}</p>{/each}
          </div>
        {/if}
      </div>
      <footer class="dialog-foot">
        <button onclick={onclose}>{done ? 'Done' : 'Cancel'}</button>
        <span class="state {wait.tone}">{wait.text}</span>
        <button class:hidden={!waiting} onclick={() => commands.connectorsCancelSignIn()}>Stop waiting</button>
        <button class="main" class:hidden={step === 2} onclick={saveClient}>Save and continue</button>
        <button class="main" class:hidden={step !== 2 || !!done} disabled={!word || waiting} onclick={signIn}>Sign in</button>
      </footer>
    </div>
  </div>
{/if}

<style>
  #connectors-oauth { display: flex; justify-content: center; padding-top: var(--space-2); }
  .dialog {
    width: 100%; max-width: 520px; border: 1px solid var(--color-separator); border-radius: var(--radius-sheet); background: var(--color-card-bg);
    display: flex; flex-direction: column; overflow: hidden; box-shadow: var(--shadow-popover);
  }
  .dialog-head { display: flex; align-items: center; gap: var(--space-3); padding: var(--space-4) var(--space-5); border-bottom: 1px solid var(--color-separator); }
  .dialog-head h1 { flex: 1; margin: 0; font-size: var(--text-title3); }
  .glyph {
    width: 28px; height: 28px; border-radius: var(--radius-control); background: var(--color-separator); display: flex; align-items: center;
    justify-content: center; font-weight: 700;
  }
  .step-count { font-size: var(--text-callout); color: var(--color-label-secondary); }
  .steps { display: flex; gap: var(--space-2); margin: 0; padding: var(--space-3) var(--space-5) 0; list-style: none; }
  .steps li { flex: 1; display: flex; align-items: center; gap: var(--space-2); font-size: var(--text-callout); color: var(--color-label-secondary); }
  .steps .n {
    width: 18px; height: 18px; border-radius: var(--radius-capsule); background: var(--color-separator); display: flex; align-items: center;
    justify-content: center; font-size: var(--text-subheadline); color: inherit;
  }
  .steps li.now { color: var(--color-label); font-weight: 600; }
  .steps li.now .n { background: var(--color-accent); color: var(--color-on-accent); }
  .steps li.done .n { background: var(--color-green); color: var(--color-on-accent); }
  .dialog-body { padding: var(--space-4) var(--space-5); display: flex; flex-direction: column; gap: var(--space-4); }
  .dialog-body .lead { margin: 0; }
  .dialog-body .small { font-size: var(--text-callout); }
  .field { display: flex; flex-direction: column; gap: var(--space-2); margin-top: var(--space-3); }
  .field span { font-weight: 600; }
  .field small { font-weight: 400; color: var(--color-label-secondary); }
  .field input { width: 100%; padding: var(--space-2) var(--space-3); border-radius: var(--radius-control); }
  .choice { border: 0; margin: 0 0 var(--space-4); padding: 0; display: flex; flex-direction: column; gap: var(--space-2); }
  .choice legend { padding: 0; margin-bottom: var(--space-2); font-weight: 600; }
  .chips { display: flex; gap: var(--space-2); }
  .chips label {
    flex: 1; display: flex; align-items: center; justify-content: center; gap: var(--space-2); height: 34px; border-radius: var(--radius-control);
    border: 1.5px solid var(--color-control-border);
  }
  .chips label:has(input:checked) { border-color: var(--color-accent); background: var(--color-accent-soft); font-weight: 600; }
  .chips input { margin: 0; accent-color: var(--color-accent); }
  .say { color: var(--color-label-secondary); }
  .say:empty { display: none; }
  .checks { border: 1px solid var(--color-separator); border-radius: var(--radius-card); overflow: hidden; }
  .checks label { display: flex; align-items: center; gap: var(--space-3); padding: var(--space-3) var(--space-4); border-top: 1px solid var(--color-separator); }
  .checks label:first-child { border-top: 0; }
  .checks input { margin: 0; accent-color: var(--color-accent); }
  .dialog-foot { display: flex; align-items: center; gap: var(--space-2); padding: var(--space-3) var(--space-5); border-top: 1px solid var(--color-separator); }
  .dialog-foot .state { flex: 1; text-align: right; }
  /* Home's page header rule, which this dialog's head does not share. */
  header.dialog-head { justify-content: flex-start; margin-bottom: 0; }
</style>
