<script lang="ts">
  // Settings → Diagnostics (specs/200-diagnostics, R-1 to R-9): how fast dictations are and what
  // failed, read when the section opens and on Refresh, never in the background.
  import { sheet } from '../home/settings.svelte';
  import { commands, type DiagnosticsView, type Window } from '../lib/ipc';
  import Report from './Report.svelte';
  import { useSettings } from './state.svelte';

  let { on }: { on: boolean } = $props();
  const s = useSettings();
  let d = $state<DiagnosticsView | null>(null);
  let error = $state('');
  /** Report a problem, open, with the History item it came from. */
  let report = $state<{ from: number | null } | null>(null);

  const ms = (us: number) => Math.round(us / 1000) + ' ms';
  const OUTCOMES: Record<string, string> = { pasted: 'Pasted', copied: 'Copied', failed: 'Failed', cancelled: 'Cancelled', nothing_heard: 'Nothing heard' };
  const STAGES: Record<string, string> = {
    mic_start: 'Microphone start',
    speech: 'Speech',
    dictionary: 'Dictionary',
    cleanup: 'Cleanup',
    insert: 'Paste',
    history_write: 'History write',
  };

  const v = $derived(d?.view);
  const off = $derived(!d || !d.shown || !d.keep || !v || v.empty);
  const failures = $derived(v?.failures.map(([kind, n]) => kind.replaceAll('_', ' ') + ' ' + n).join(', ') ?? '');

  async function load() {
    try {
      d = await commands.diagnosticsView();
      error = d.error ?? '';
    } catch (e) {
      error = String(e);
    }
  }

  // Each visit to the section reads it again; `diagnostics/report[/<history id>]` (Home's Help, a
  // failed item, the crash notice) opens Report a problem.
  $effect(() => {
    void sheet.visit;
    if (s.section !== 'diagnostics') return;
    load();
    if (s.item?.startsWith('report')) report = { from: Number(s.item.split('/')[1]) || null };
  });
</script>

{#snippet cells(w: Window)}
  {#if w.state === 'too_few'}
    <span class="faint">{w.count === 1 ? '1 dictation: too few to measure' : w.count + ' dictations: too few to measure'}</span>
  {:else}
    <!-- Spaces at a span's edge are written as text, which Svelte keeps. -->
    <span>{`median ${ms(w.p50_us)} · 90% `}</span><span class={w.over_budget ? 'state bad' : ''}>{ms(w.p90_us)}</span><span class="faint"
      >{` · ${w.count} dictations`}</span
    >
  {/if}
{/snippet}

<section id="diagnostics" class:on>
  <div class="row" style="border:0;padding-top:0">
    <div>
      <h1>Diagnostics</h1>
      <p class="lead" style="margin:0">How fast your dictations are and what failed, from the timing traces on this Mac. Nothing is sent.</p>
    </div>
    <span style="white-space:nowrap"
      ><button onclick={load}>Refresh</button>
      <button class="main" data-words="report problem bug issue" onclick={() => (report = { from: null })}>Report a problem…</button></span
    >
  </div>
  <p class="lead" class:hidden={!d || !off} data-words="diagnostics traces off empty">
    {#if d && off}
      {!d.keep ? 'Timing traces are off, so there is nothing to show. ' : 'No dictations in the last 7 days yet. '}<a
        class="link"
        href="#privacy"
        onclick={(e) => (e.preventDefault(), s.go('privacy'))}>Keep timing traces</a
      >
    {/if}
  </p>
  <div class:hidden={off}>
    {#if v && !off}
      <p class="lead" style="margin:var(--space-4) 0 var(--space-1)">Key release to text</p>
      <div data-words="latency speed key release text median slow budget">
        <div class="row"><div class="what">Last 24 hours</div><span>{@render cells(v.last_day)}</span></div>
        <div class="row"><div class="what">Last 7 days</div><span>{@render cells(v.last_week)}</span></div>
      </div>
      <p class="state bad" class:hidden={!v.slowest_stage}>
        {v.slowest_stage ? `Over the 800 ms budget. The slowest stage is ${(STAGES[v.slowest_stage] ?? v.slowest_stage).toLowerCase()}.` : ''}
      </p>
      <p class="lead" style="margin:var(--space-4) 0 var(--space-1)">Stages, last 7 days</p>
      <table class="diag" data-words="stages speech cleanup provider model median">
        <thead><tr><th>Stage</th><th>Provider</th><th class="n">Median</th><th class="n">90%</th><th class="n">Dictations</th></tr></thead>
        <tbody>
          {#each v.stages as st, i (i)}
            <tr>
              <td>{STAGES[st.stage] ?? st.stage}</td>
              <td>{st.provider ? st.provider + ' · ' + st.model : ''}</td>
              <td class="n">{st.window.state === 'ready' ? ms(st.window.p50_us) : '—'}</td>
              <td class="n">{st.window.state === 'ready' ? ms(st.window.p90_us) : '—'}</td>
              <td class="n">{st.window.count}</td>
            </tr>
          {/each}
        </tbody>
      </table>
      <p class="lead" style="margin:var(--space-4) 0 var(--space-1)">Outcomes, last 7 days</p>
      <div data-words="outcomes pasted copied failed cancelled failures errors">
        <div class="row"><div class="what">{v.outcomes.map(([o, n]) => (OUTCOMES[o] ?? o) + ' ' + n).join(' · ')}</div></div>
        {#if failures}
          <div class="row"><div class="what">Failures by kind<small>{failures}</small></div></div>
        {/if}
      </div>
    {/if}
  </div>
  <p class="lead" style="margin:var(--space-4) 0 var(--space-1)">Warnings and errors</p>
  <pre id="diag-warnings" data-words="warnings errors log">{d ? (d.warnings.length ? d.warnings.join('\n') : 'None.') : ''}</pre>
  <p class="state bad">{error}</p>
</section>

{#if report}
  <Report from={report.from} onclose={() => (report = null)} />
{/if}

<style>
  .diag { width: 100%; border-collapse: collapse; margin-top: var(--space-2); }
  .diag th {
    text-align: left; font-weight: 500; color: var(--color-label-secondary); font-size: var(--text-callout);
    padding: var(--space-2) var(--space-2) var(--space-2) 0; border-bottom: 1px solid var(--color-separator);
  }
  .diag td { padding: var(--space-2) var(--space-2) var(--space-2) 0; border-bottom: 1px solid var(--color-separator); }
  .diag td.n { text-align: right; font-variant-numeric: tabular-nums; }
  #diag-warnings {
    font: var(--text-subheadline) var(--font-mono); white-space: pre-wrap; color: var(--color-label-secondary);
    margin: var(--space-2) 0 0; max-height: 240px; overflow: auto;
  }
</style>
