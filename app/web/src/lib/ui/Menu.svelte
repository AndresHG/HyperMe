<script lang="ts" module>
  /** One action in a menu, with its key shown on the right. */
  export interface MenuItem {
    id: string;
    label: string;
    key?: string;
  }
</script>

<script lang="ts">
  // Menu: a popover list of actions at a point (⌘K on a row). While it is open it takes the
  // arrow keys, Enter and Esc before the page does; a click outside closes it. It never takes
  // focus, so the field or row the user was on keeps it.
  let {
    items,
    open = $bindable(false),
    x = 0,
    y = 0,
    label = 'Actions',
    onpick,
  }: { items: MenuItem[]; open?: boolean; x?: number; y?: number; label?: string; onpick: (id: string) => void } =
    $props();

  let at = $state(0);

  $effect(() => {
    if (open) at = 0;
  });

  function pick(i: number) {
    const item = items[i];
    open = false;
    if (item) onpick(item.id);
  }

  function onkeydown(e: KeyboardEvent) {
    if (!open) return;
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      at = (at + (e.key === 'ArrowDown' ? 1 : items.length - 1)) % Math.max(1, items.length);
    } else if (e.key === 'Enter') {
      pick(at);
    } else if (e.key === 'Escape' || (e.metaKey && e.key === 'k')) {
      open = false;
    } else {
      return;
    }
    e.preventDefault();
    e.stopImmediatePropagation();
  }
</script>

<svelte:window onkeydowncapture={onkeydown} onmousedown={() => (open = false)} />

{#if open}
  <div class="menu" role="menu" aria-label={label} style:left="{x}px" style:top="{y}px">
    {#each items as item, i (item.id)}
      <div
        role="menuitem"
        tabindex="-1"
        class:on={i === at}
        onmouseenter={() => (at = i)}
        onmousedown={(e) => {
          e.preventDefault();
          e.stopPropagation();
          pick(i);
        }}
      >
        {item.label}{#if item.key}<kbd>{item.key}</kbd>{/if}
      </div>
    {/each}
  </div>
{/if}

<style>
  .menu {
    position: fixed; z-index: 10; min-width: 200px; padding: var(--space-1); border-radius: var(--radius-card);
    border: var(--border-width) solid var(--color-separator); background: var(--color-content-bg); box-shadow: var(--shadow-popover);
  }
  [role='menuitem'] {
    display: flex; justify-content: space-between; gap: var(--space-5);
    padding: var(--space-2) var(--space-2); border-radius: var(--radius-control);
  }
  [role='menuitem'].on { background: var(--color-accent-selected); }
  kbd { font: var(--text-subheadline) var(--font-system); color: var(--color-label-secondary); }
</style>
