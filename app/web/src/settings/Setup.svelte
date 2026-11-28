<script lang="ts">
  // Setup check: what HyperMe needs before dictation works, and every fallback whose key went
  // missing (specs/96, R-24a), and the connector accounts that need you.
  import { commands } from '../lib/ipc';
  import ConnectorRows from './connectors/ConnectorRows.svelte';
  import StateMark from './StateMark.svelte';
  import { mark, useSettings } from './state.svelte';

  let { on }: { on: boolean } = $props();
  const s = useSettings();
  const v = $derived(s.view);
  const host = $derived(v?.setup.host_app);
</script>

<section id="setup" class:on>
  <h1>Setup check</h1>
  <p class="lead">What HyperMe needs before dictation works.</p>
  <p class="lead" id="host-note">
    {host ? `HyperMe is running from ${host}, so macOS lists ${host} instead of HyperMe: turn ${host} on, then quit it (⌘Q) and open it again.` : ''}
  </p>
  <div class="row" data-words="accessibility paste permission">
    <div class="what">Accessibility<small>Needed to paste at the cursor.</small></div>
    <span><StateMark mark={v && mark(v.setup.accessibility, 'Allowed', 'Not allowed')} /> <button onclick={() => commands.settingsOpen('request-accessibility')}>Allow</button> <button onclick={() => commands.settingsOpen('accessibility')}>Open</button></span>
  </div>
  <div class="row" data-words="input monitoring hotkey keyboard permission">
    <div class="what">Input Monitoring<small>Needed to see the hotkey.</small></div>
    <span><StateMark mark={v && mark(v.setup.input_monitoring, 'Allowed', 'Not allowed')} /> <button onclick={() => commands.settingsOpen('request-input-monitoring')}>Allow</button> <button onclick={() => commands.settingsOpen('input-monitoring')}>Open</button></span>
  </div>
  <div class="row" data-words="microphone permission">
    <div class="what">Microphone<small>macOS asks the first time you dictate.</small></div>
    <button onclick={() => commands.settingsOpen('microphone')}>Open</button>
  </div>
  <div class="row" data-words="speech key speechmatics">
    <div class="what">Speech key<small>{v?.config?.speech_provider ?? ''}</small></div>
    <StateMark mark={v && mark(v.keys.speech, 'Key saved', 'No key')} />
  </div>
  <div class="row" data-words="cleanup key groq">
    <div class="what">Cleanup key<small>{v?.config?.cleanup_provider ?? ''}</small></div>
    <StateMark mark={v && mark(v.keys.cleanup, 'Key saved', 'No key')} />
  </div>
  <div>
    {#each v?.roles ?? [] as r (r.role)}
      {#if r.fallback}
        <div class="row">
          <div class="what">{r.label} fallback<small>{r.fallback.name} · {r.fallback.model}</small></div>
          <StateMark mark={mark(r.fallback.ready, 'Key saved', 'No key: skipped until you add it')} />
        </div>
      {/if}
    {/each}
  </div>
  <div>
    <ConnectorRows rows={s.connectors?.setup ?? []} words="connector account setup" onpick={(r) => commands.connectorsOpenLink(r.link).catch(s.fail)} />
  </div>
</section>
