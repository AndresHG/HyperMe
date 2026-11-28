<script lang="ts">
  // The sidebar (R-5, R-6): the sections with their ⌘ keys, the dot when something waits on
  // Home, the dictation hint, and Settings, Help and Report a problem.
  import type { Section } from '../lib/ipc';
  import Icon from './Icon.svelte';
  import { openSettings } from './settings.svelte';

  let { sections, current, attention }: { sections: Section[]; current: string; attention: boolean } = $props();


  function open(e: MouseEvent, section: string) {
    e.preventDefault();
    openSettings(section);
  }
  // ⌘, opens Settings at the section closest to where you are (docs/UX.md).
  const settingsSection = $derived(current === 'history' ? 'privacy' : 'general');
</script>

<nav aria-label="Sections">
  <div class="brand"><i aria-hidden="true"></i>HyperMe</div>
  <div id="sections">
    {#each sections as s (s.id)}
      <a href="#{s.id}" data-id={s.id} class:on={s.id === current}>
        <Icon name={s.id} />
        <span class="label">{s.title}</span>
        {#if s.id === 'home' && attention}<span class="dot" aria-label="Something is waiting for you"></span>{/if}
        <kbd>{s.shortcut}</kbd>
      </a>
    {/each}
  </div>
  <div class="spacer"></div>
  <p class="hint">Hold <kbd class="key">fn</kbd> to dictate. Say "Hyper, …" for a command.</p>
  <div class="bottom">
    <a href="#settings" id="open-settings" onclick={(e) => open(e, settingsSection)}><Icon name="settings" /><span class="label">Settings</span><kbd>⌘,</kbd></a>
    <a href="#help" id="open-help" onclick={(e) => open(e, 'about')}><Icon name="help" /><span class="label">Help</span></a>
    <!-- Spec 200, R-12: Report a problem from Help, and from a failed item. -->
    <a href="#report" id="open-report" onclick={(e) => open(e, 'diagnostics/report')}><Icon name="report" /><span class="label">Report a problem</span></a>
  </div>
</nav>

<style>
  nav { background: var(--color-sidebar-bg); border-right: 1px solid var(--color-separator); padding: calc(var(--title-bar-height) + var(--space-3)) var(--space-3) var(--space-3); display: flex; flex-direction: column; gap: var(--space-1); }
  .brand { display: flex; align-items: center; gap: var(--space-2); padding: 0 var(--space-2) var(--space-4); font: 700 var(--text-title3) var(--font-system); }
  .brand i { width: 22px; height: 12px; border-radius: var(--radius-capsule); background: var(--color-pill-bg); box-shadow: 0 0 0 2px var(--color-accent); }
  a { display: flex; align-items: center; gap: var(--space-3); padding: var(--space-2) var(--space-2); border-radius: var(--radius-control); color: inherit; text-decoration: none; cursor: default; }
  a :global(svg) { flex: none; }
  a span.label { flex: 1; }
  a:hover { background: var(--color-hover); }
  a.on { background: var(--color-accent-selected); font-weight: 600; }
  a kbd { font: var(--text-subheadline) var(--font-system); color: var(--color-label-secondary); font-weight: 400; }
  a .dot { width: 7px; height: 7px; border-radius: var(--radius-capsule); background: var(--color-accent); }
  .spacer { flex: 1; }
  .hint { margin: 0 var(--space-1) var(--space-3); padding: var(--space-3); border-radius: var(--radius-card); background: var(--color-card-bg); border: 1px solid var(--color-separator); font-size: var(--text-callout); color: var(--color-label-secondary); line-height: 1.45; }
  .bottom a { color: var(--color-label-secondary); }
  kbd.key { display: inline-block; padding: 0 var(--space-1); border: 1px solid var(--color-separator); border-radius: var(--radius-small); font: var(--text-subheadline) var(--font-system); color: var(--color-label); }
</style>
