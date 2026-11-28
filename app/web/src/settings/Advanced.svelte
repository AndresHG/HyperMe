<script lang="ts">
  // Advanced: deadlines, thresholds and endpoints, shown with "Show advanced" in General.
  import { commands } from '../lib/ipc';
  import Setting from './Setting.svelte';
  import { useSettings } from './state.svelte';

  let { on }: { on: boolean } = $props();
  const s = useSettings();
</script>

<section id="advanced" class:on>
  <h1>Advanced</h1>
  <p class="lead">For tuning. Everything is also in the config file.</p>
  <div class="row" data-words="cleanup deadline raw">
    <div class="what">Cleanup deadline (ms)<small>After it, the raw transcript is pasted.</small></div>
    <Setting section="cleanup" key="deadline_ms" field="deadline_ms" type="number" min={100} max={10000} />
  </div>
  <div class="row" data-words="minimum hold tap">
    <div class="what">Minimum hold (ms)</div>
    <Setting section="dictation" key="min_hold_ms" field="min_hold_ms" type="number" min={50} max={2000} />
  </div>
  <div class="row" data-words="maximum length">
    <div class="what">Maximum length (s)</div>
    <Setting section="dictation" key="max_length_s" field="max_length_s" type="number" min={5} max={600} />
  </div>
  <div class="row" data-words="short dictation words">
    <div class="what">Short dictation (words)<small>Fewer words skip cleanup.</small></div>
    <Setting section="dictation" key="short_dictation_words" field="short_dictation_words" type="number" min={0} max={50} />
  </div>
  <div class="row" data-words="speech endpoint url region">
    <div class="what">Speech endpoint<small>Overrides the provider's own address, such as another region.</small></div>
    <Setting section="speech" key="endpoint" field="speech_endpoint" type="text" />
  </div>
  <div class="row" data-words="cleanup endpoint url proxy">
    <div class="what">Cleanup endpoint<small>Overrides the provider's own address. A new host is better added as a custom endpoint in Providers.</small></div>
    <Setting section="cleanup" key="endpoint" field="cleanup_endpoint" type="text" />
  </div>
  <div class="row" data-words="config file toml">
    <div class="what">Config file<small>{s.view?.config_path ?? ''}</small></div>
    <button onclick={() => commands.settingsOpen('config')}>Open config file</button>
  </div>
</section>
