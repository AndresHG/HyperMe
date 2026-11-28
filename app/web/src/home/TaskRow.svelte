<script lang="ts">
  // One task in Today: the round check, the title (double-click renames), its tools on hover,
  // and its marks.
  import { tick } from 'svelte';
  import type { LineRow, TaskAction } from '../lib/ipc';

  let { line, act }: { line: LineRow; act: (action: TaskAction, value?: string) => void } = $props();

  let renaming = $state(false);
  let name = $state('');
  let input = $state<HTMLInputElement>();
  let done = false;

  async function rename() {
    name = line.title;
    done = false;
    renaming = true;
    await tick();
    input?.focus();
    input?.select();
  }

  function finish(save: boolean) {
    if (done) return;
    done = true;
    renaming = false;
    if (save && name.trim() && name.trim() !== line.title) act('rename', name);
  }

  const tone = (mark: string) => (mark === 'new' ? 'new' : mark === 'overdue' || mark.startsWith('due') ? 'warn' : '');
</script>

<div class="task">
  <button class="check" aria-label="Done: {line.title}" onclick={() => act('done')}></button>
  {#if renaming}
    <input
      class="name"
      bind:this={input}
      bind:value={name}
      aria-label="Rename {line.title}"
      onkeydown={(e) => {
        if (e.key === 'Enter') {
          e.preventDefault();
          finish(true);
        } else if (e.key === 'Escape') {
          e.preventDefault();
          finish(false);
        }
      }}
      onblur={() => finish(true)}
    />
  {:else}
    <!-- The Rename button does the same from the keyboard. -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <span class="title" title="Double-click to rename" ondblclick={rename}>{line.title}</span>
  {/if}
  <span class="acts">
    <button onclick={() => act('tomorrow')}>Tomorrow</button>
    <button onclick={rename}>Rename</button>
    <button onclick={() => act('drop')}>Drop</button>
  </span>
  {#each line.marks as mark, i (i)}<span class="tag {tone(mark)}">{mark}</span>{/each}
</div>

<style>
  .task { display: flex; align-items: center; gap: var(--space-3); padding: var(--space-2); border-radius: var(--radius-control); min-height: 38px; }
  .task:hover { background: var(--color-hover); }
  .check { width: 18px; height: 18px; padding: 0; border-radius: var(--radius-capsule); border: 1.5px solid var(--color-label-secondary); background: transparent; flex: none; }
  .check:hover { border-color: var(--color-accent); background: var(--color-accent-selected); }
  .title { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .acts { display: none; gap: var(--space-1); }
  .task:hover .acts, .task:focus-within .acts { display: flex; }
  .acts button { font-size: var(--text-callout); padding: var(--space-1) var(--space-2); border-radius: var(--radius-control); border: 1px solid var(--color-separator); background: var(--color-content-bg); }
  .acts button:hover { border-color: var(--color-accent); }
  input { font: inherit; padding: var(--space-1) var(--space-2); border-radius: var(--radius-control); border: 1px solid var(--color-accent); background: var(--color-content-bg); color: inherit; flex: 1; outline: none; }
</style>
