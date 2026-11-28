<script lang="ts">
  // General: everyday preferences, and the microphone order (#221): the user's order, the first
  // connected one wins, else the system default.
  import { commands, type MicrophonesView } from '../lib/ipc';
  import Setting from './Setting.svelte';
  import { useSettings } from './state.svelte';

  let { on }: { on: boolean } = $props();
  const s = useSettings();
  let mics = $state<MicrophonesView | null>(null);
  const names = $derived(mics?.microphones.map((m) => m.name) ?? []);

  async function load() {
    try {
      mics = await commands.microphonesList();
    } catch {
      // No microphone list: the row stays as it is.
    }
  }

  async function saveMics(order: string[]) {
    try {
      await commands.microphonesSet(order);
      s.error = '';
    } catch (e) {
      s.fail(e);
    }
    load();
  }

  function move(from: number, to: number) {
    const order = names.slice();
    order.splice(to, 0, ...order.splice(from, 1));
    saveMics(order);
  }

  // The order is read with the rest of the settings.
  $effect(() => {
    if (s.view?.config) load();
  });
</script>

<section id="general" class:on>
  <h1>General</h1>
  <p class="lead">Everyday preferences.</p>
  <div class="card-rows">
    <div class="row" data-words="sounds start stop">
      <div class="what">Start and stop sounds<small>A soft sound when recording starts and stops.</small></div>
      <Setting section="dictation" key="sounds" field="sounds" />
    </div>
    <div class="row" data-words="mute audio music media sound video while dictating">
      <div class="what">Mute audio while dictating<small>Music and videos go quiet while you talk and come back when you stop.</small></div>
      <Setting section="dictation" key="mute_while_dictating" field="mute_while_dictating" />
    </div>
    <div class="row" data-words="microphone mic input airpods usb order priority default">
      <div class="what">
        Microphone<small>{mics ? (mics.ordered ? 'Each dictation uses the first connected microphone in this order.' : 'The system default. Move a microphone up to prefer it.') : ''}</small>
      </div>
      <button class:hidden={!mics?.ordered} onclick={() => saveMics([])}>Use system default</button>
    </div>
    <div id="mic-list">
      {#each mics?.microphones ?? [] as m, i (m.name)}
        <div class="row" data-words="microphone mic input order">
          <div class="what">{m.name}<small>{!m.connected ? 'Not connected' : m.default ? 'System default' : ''}</small></div>
          <span>
            {#if !m.connected}<button onclick={() => saveMics(names.filter((n) => n !== m.name))}>Forget</button> {/if}<button
              title="Prefer"
              disabled={i === 0}
              onclick={() => move(i, i - 1)}>↑</button
            > <button title="Prefer less" disabled={i === names.length - 1} onclick={() => move(i, i + 1)}>↓</button>
          </span>
        </div>
      {/each}
    </div>
    <div class="row" data-words="bead dot idle show while idle bottom">
      <div class="what">Show while idle<small>A small still bead at the bottom centre; it grows into the pill when you dictate.</small></div>
      <Setting section="dictation" key="show_while_idle" field="show_while_idle" />
    </div>
    <div class="row" data-words="advanced">
      <div class="what">Show advanced<small>Deadlines, thresholds, endpoints and the config file.</small></div>
      <input type="checkbox" bind:checked={s.advanced} />
    </div>
  </div>
</section>

<style>
  #mic-list { display: contents; }
  #mic-list > .row {
    border: 1px solid var(--color-separator); border-top: 0; padding: var(--space-2) var(--space-4) var(--space-2) var(--space-8);
    background: var(--color-card-bg);
  }
</style>
