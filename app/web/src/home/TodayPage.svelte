<script lang="ts">
  // The Home page (R-7 to R-9): the greeting with the date and two numbers, a line while
  // dictionary suggestions wait, other features' blocks, then the day.
  import { onDestroy, onMount, tick } from 'svelte';
  import { commands, on, tell, type Block } from '../lib/ipc';
  import { LOCALE, dayFormat, duration, greeting } from './format';
  import Today from './Today.svelte';

  let { block }: { block: string | null } = $props();

  const hello = greeting();
  const date = dayFormat.format(new Date());
  let stats = $state(' ');
  let suggested = $state(0);
  let blocks = $state<Block[]>([]);
  let today: Today;

  // R-7: the date and the two numbers, in one quiet line under the greeting.
  async function loadStats() {
    try {
      const s = await commands.homeStats(-new Date().getTimezoneOffset());
      stats = `${date} · ${s.words_this_week.toLocaleString(LOCALE)} words this week · ${duration(s.seconds_saved)} saved`;
    } catch {
      stats = date;
    }
  }

  // A block once it has been on screen clears what was new in it (R-22).
  const seen = new IntersectionObserver((entries) => {
    for (const entry of entries) {
      if (entry.isIntersecting) {
        tell.blockViewed((entry.target as HTMLElement).dataset.block ?? '');
        seen.unobserve(entry.target);
      }
    }
  });
  const observe = (node: HTMLElement) => {
    seen.observe(node);
  };

  /** Reads everything on the page again (`home://refresh`). */
  export async function load() {
    await loadStats();
    // Spec 93, R-22: one line while dictionary suggestions wait; the pill shows nothing.
    suggested = await commands.dictionarySuggested().catch(() => 0);
    blocks = await commands.homeBlocks();
    await today?.load();
    if (block) {
      await tick();
      document.getElementById('block-' + block)?.scrollIntoView();
    }
  }

  // A dictation made while Home is open changes the numbers (AC-15).
  const stop = on('home://changed', () => loadStats());
  onMount(() => {
    // The day on screen clears what was new there (R-21, R-22).
    tell.blockViewed('today');
    load();
  });
  onDestroy(() => {
    seen.disconnect();
    stop.then((off) => off());
  });
</script>

<section id="home" aria-labelledby="greeting">
  <header>
    <div>
      <h1 id="greeting">{hello}</h1>
      <p class="lead" id="stats">{stats}</p>
    </div>
    <p class="lead">Say "Hyper, add a task…"</p>
  </header>
  {#if suggested}
    <p class="suggested-line" id="suggested-line">
      {suggested === 1 ? '1 suggested word' : suggested + ' suggested words'} · <a href="#dictionary">Review</a>
    </p>
  {/if}
  {#if blocks.length}
    <div id="blocks">
      {#each blocks as b (b.id)}
        <div id="block-{b.id}" data-block={b.id} use:observe><h2>{b.title}</h2></div>
      {/each}
    </div>
  {/if}
  <Today bind:this={today} />
</section>

<style>
  .suggested-line { margin: 0 0 var(--space-4); color: var(--color-label-secondary); }
  .suggested-line a { color: var(--color-accent-text); }
  #blocks { margin-bottom: var(--space-5); }
</style>
