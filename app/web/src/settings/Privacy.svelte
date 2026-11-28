<script lang="ts">
  // Privacy: where audio and text go (specs/96, R-29: every provider set, fallbacks included, and
  // what each one receives), what stays on this Mac, and the connected accounts' rules.
  import { commands, type Destination, type SamplesView } from '../lib/ipc';
  import { Button } from '../lib/ui';
  import Setting from './Setting.svelte';
  import { useSettings } from './state.svelte';

  let { on }: { on: boolean } = $props();
  const s = useSettings();
  const config = $derived(s.view?.config);
  const privacy = $derived(s.connectors?.privacy);
  const retention = $derived(config?.retention_days ?? null);

  const SENT = { speech: 'Your audio', cleanup: 'The transcript, the app name and your dictionary' };
  const what = (d: Destination) => (d.on_this_mac ? 'Stays on this Mac' : `${SENT[d.role]} go${d.role === 'speech' ? 'es' : ''} to ${d.name}`);
  const when = (d: Destination) => (d.fallback_for ? `Only if ${d.fallback_for} fails` : d.role === 'speech' ? 'Dictation speech' : 'Dictation cleanup');

  // Spec 267, R-12: corrected dictations kept as ground truth, outside the history's retention.
  let samples = $state<SamplesView | null>(null);
  let confirmDelete = $state(false);
  const loadSamples = () => commands.samplesView().then((v) => (samples = v)).catch(() => (samples = null));
  $effect(() => {
    if (on) loadSamples();
  });
  const size = (bytes: number) => (bytes < 1_000_000 ? `${Math.ceil(bytes / 1000)} KB` : `${(bytes / 1_000_000).toFixed(1)} MB`);
  async function deleteSamples() {
    confirmDelete = false;
    await commands.samplesDeleteAll().catch(() => {});
    await loadSamples();
  }

  function keep(e: Event & { currentTarget: HTMLSelectElement }) {
    const v = e.currentTarget.value;
    s.save('history', 'retention_days', v === 'unlimited' ? 'unlimited' : Number(v));
  }
</script>

<section id="privacy" class:on>
  <h1>Privacy</h1>
  <p class="lead">
    Audio and text go only to the providers listed here, under your keys. Connected accounts follow the rules at the bottom of this page. There is no
    telemetry. A dictation's audio stays on this Mac so a dropped connection never loses it.
  </p>
  <div data-words="providers speech cleanup fallback where audio text goes privacy">
    {#each s.view?.destinations ?? [] as d (d.role + d.name + d.fallback_for)}
      <div class="row">
        <div class="what">{d.name}{d.fallback_for ? ' (fallback)' : ''}<small>{when(d)} · {d.model}</small></div>
        <span class="faint">{what(d)}</span>
      </div>
    {/each}
  </div>
  <div class="row" data-words="history dictation audio keep">
    <div class="what">Dictation history<small>Keeps each dictation's audio and text on this Mac.</small></div>
    <Setting section="history" key="enabled" field="history" />
  </div>
  <div class="row" data-words="history retention days unlimited">
    <div class="what">Keep for<small>Older dictations are deleted.</small></div>
    <select value={retention === null ? 'unlimited' : String(retention)} onchange={keep}>
      <option value="30">30 days</option>
      <option value="unlimited">Unlimited</option>
      {#if retention !== null && retention !== 30}<option value={String(retention)}>{retention} days</option>{/if}
    </select>
  </div>
  <div class="row" data-words="history size limit">
    <div class="what">Size limit (MB)<small>The oldest dictations go first.</small></div>
    <Setting section="history" key="size_limit_mb" field="size_limit_mb" type="number" min={10} />
  </div>
  <div class="row" data-words="ground truth golden samples corrected benchmark keep audio">
    <div class="what">
      Ground truth<small
        >Dictations you corrected in History, kept with their audio for benchmarks until you delete them.{#if samples?.count}
          {samples.count} {samples.count === 1 ? 'dictation' : 'dictations'}, {size(samples.bytes)}.{/if}</small
      >
    </div>
    <span class="actions">
      <Button size="small" onclick={() => commands.samplesReveal()}>Show in Finder</Button>
      {#if confirmDelete}
        <Button size="small" onclick={deleteSamples}>Delete {samples?.count ?? 0}</Button>
        <Button size="small" variant="borderless" onclick={() => (confirmDelete = false)}>Keep</Button>
      {:else}
        <Button size="small" disabled={!samples?.count} onclick={() => (confirmDelete = true)}>Delete all</Button>
      {/if}
    </span>
  </div>
  <div class="row" data-words="timing traces stages measure keep">
    <div class="what">Keep timing traces<small>Stage timings of each dictation for 30 days, even with history off. Numbers only, never text.</small></div>
    <Setting section="history" key="keep_traces" field="keep_traces" />
  </div>
  <div class="row" data-words="timing log measure">
    <div class="what">Timing log<small>One line of stage timings per dictation, numbers only.</small></div>
    <Setting section="timing" key="log" field="timing_log" />
  </div>
  <div class:hidden={!privacy?.accounts.length} data-words="connected accounts google calendar mail privacy">
    <p class="lead" style="margin:var(--space-4) 0 var(--space-1)">Connected accounts</p>
    <div>
      {#each privacy?.accounts ?? [] as a (a.title)}
        <div class="row" data-words="connected account {a.title}">
          <div class="what">{a.title}<small>{a.reads.join(' · ')}</small><small>{a.approvals || ''}</small></div>
        </div>
      {/each}
    </div>
    <p class="lead" style="margin:var(--space-2) 0 0">{privacy?.where_it_goes ?? ''}</p>
    <p class="lead" style="margin:var(--space-1) 0 0">{privacy?.kept ?? ''}</p>
  </div>
</section>

<style>
  .actions { display: flex; gap: var(--space-2); flex: none; }
</style>
