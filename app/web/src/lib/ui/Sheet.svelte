<script lang="ts">
  // Sheet: a dialog inside the window, over a dimmed page (Settings in Home). It is part of the
  // page, inside the window's native frame, so macOS never draws a square shadow around it
  // (docs/DESIGN.md). A click on the dimmed page or Esc closes it. Its content exists only while
  // it is open.
  import type { Snippet } from 'svelte';

  let {
    open = $bindable(false),
    label,
    size = 'large',
    onclose,
    children,
  }: {
    open?: boolean;
    label: string;
    /** `large` for a page such as Settings; `alert` for a short question with its buttons. */
    size?: 'large' | 'alert';
    onclose?: () => void;
    children: Snippet;
  } = $props();

  function close() {
    open = false;
    onclose?.();
  }
</script>

<svelte:window onkeydown={(e) => open && e.key === 'Escape' && !e.defaultPrevented && close()} />

{#if open}
  <div class="dim" role="presentation" onmousedown={close}></div>
  <div class="sheet" class:alert={size === 'alert'} role="dialog" aria-modal="true" aria-label={label}>{@render children()}</div>
{/if}

<style>
  .dim { position: fixed; inset: 0; background: var(--color-scrim); z-index: 20; }
  .sheet {
    position: fixed; z-index: 21; left: 50%; top: calc(50% + var(--title-bar-height) / 2); transform: translate(-50%, -50%);
    width: clamp(640px, calc(100vw - 2 * var(--space-8)), 1040px);
    height: clamp(420px, calc(100vh - var(--title-bar-height) - 2 * var(--space-8)), 760px);
    border-radius: var(--radius-sheet); overflow: hidden; box-shadow: var(--shadow-sheet); background: var(--color-content-bg);
  }
  .sheet.alert { width: min(420px, calc(100vw - 2 * var(--space-8))); height: auto; padding: var(--space-5); }
</style>
