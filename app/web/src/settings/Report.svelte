<script lang="ts">
  // Report a problem (specs/200-diagnostics, R-12 to R-16, R-24): the whole bundle, built on this
  // Mac and read before anything happens; then Save, or Open issue.
  import { onMount } from 'svelte';
  import { commands, type ReportView } from '../lib/ipc';

  let { from, onclose }: { from: number | null; onclose: () => void } = $props();
  let report = $state<ReportView | null>(null);
  let description = $state('');
  let crashes = $state(true);
  let note = $state('');
  let timer: ReturnType<typeof setTimeout> | undefined;

  async function build() {
    try {
      report = await commands.reportBuild(description, crashes, from);
    } catch (e) {
      note = String(e);
      return;
    }
    note = report.trimmed.length ? 'Trimmed to 1 MB: ' + report.trimmed.join(', ') + '.' : '';
  }

  function typed() {
    clearTimeout(timer);
    timer = setTimeout(build, 400);
  }

  async function save() {
    if (!report) return;
    try {
      const path = await commands.reportSave(report.text);
      if (path) note = 'Saved to ' + path + '.';
    } catch (e) {
      note = String(e);
    }
  }

  async function issue() {
    if (!report) return;
    try {
      const path = await commands.reportOpenIssue(report.text, report.url);
      note = 'Saved to ' + path + '. Attach it to the issue in your browser.';
    } catch (e) {
      note = String(e);
    }
  }

  onMount(() => {
    build();
    return () => clearTimeout(timer);
  });
</script>

<div id="report" role="dialog" aria-label="Report a problem">
  <div>
    <h1>Report a problem</h1>
    <p class="lead" style="margin:0">
      This is everything in the report. Nothing is sent: Open issue opens a GitHub page in your browser and shows the saved file for you to attach.
    </p>
    <textarea placeholder="What happened? (optional, goes in the file)" bind:value={description} oninput={typed}></textarea>
    <label class:hidden={!report?.has_reports}><input type="checkbox" bind:checked={crashes} onchange={build} /> Include macOS crash reports</label>
    <pre>{report ? report.text : 'Building…'}</pre>
    <p class="state">{note}</p>
    <div class="row" style="border:0;padding:0">
      <button onclick={onclose}>Cancel</button><span><button onclick={save}>Save…</button> <button class="main" onclick={issue}>Open issue</button></span>
    </div>
  </div>
</div>

<style>
  /* Fixed inside the sheet, which is its containing block, so it covers the sheet only. */
  #report { position: fixed; inset: 0; background: var(--color-scrim); display: flex; align-items: center; justify-content: center; z-index: 10; }
  #report > div {
    background: var(--color-content-bg); border: 1px solid var(--color-separator); border-radius: var(--radius-sheet);
    width: min(760px, 92%); max-height: 88%; display: flex; flex-direction: column; gap: var(--space-2);
    padding: var(--space-4) var(--space-5); box-shadow: var(--shadow-sheet);
  }
  textarea {
    font: inherit; padding: var(--space-2) var(--space-2); border-radius: var(--radius-control); border: 1px solid var(--color-separator);
    background: var(--color-content-bg); color: inherit; min-height: 54px; resize: vertical;
  }
  pre {
    font: var(--text-subheadline) var(--font-mono); white-space: pre-wrap; overflow: auto; flex: 1; min-height: 200px; margin: 0;
    padding: var(--space-2); border: 1px solid var(--color-separator); border-radius: var(--radius-control); background: var(--color-sidebar-bg);
  }
</style>
