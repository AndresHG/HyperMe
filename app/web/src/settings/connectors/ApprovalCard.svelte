<script lang="ts">
  // The approval card (specs/166, slice 3): what a connector server asks while a check or a Review
  // runs, above the view. Drawn with text only (R-11); ↵ picks the default, 1 to n a choice, Esc
  // nothing. The message lives only while the card does (R-20).
  import { onMount, tick } from 'svelte';
  import { commands, on, type ApprovalCard, type Span } from '../../lib/ipc';
  import { useSettings } from '../state.svelte';

  const s = useSettings();
  let card = $state<ApprovalCard | null>(null);
  let shownAt = 0;
  let node = $state<HTMLElement>();

  /** The message as paragraphs and lists of runs. */
  type Block = { list: false; runs: Span[] } | { list: true; items: Span[][] };
  const blocks = $derived.by(() => {
    const out: Block[] = [];
    let target: Span[] | null = null;
    let list: Span[][] | null = null;
    for (const span of card?.message ?? []) {
      if (span.kind === 'break') {
        target = null;
        list = null;
      } else if (span.kind === 'item') {
        if (!list) {
          list = [];
          out.push({ list: true, items: list });
        }
        target = [];
        list.push(target);
      } else {
        if (!target) {
          target = [];
          out.push({ list: false, runs: target });
          list = null;
        }
        target.push(span);
      }
    }
    return out;
  });

  function close(id?: number) {
    if (!card || (id !== undefined && card.id !== id)) return;
    card = null;
  }

  function answer(value: string | null | undefined) {
    if (!card) return;
    const id = card.id;
    close();
    commands
      .connectorsAnswer(id, value ?? null)
      .then((waiting) => {
        if (!waiting) s.fail('Took too long: Review to ask again.');
      })
      .catch(s.fail);
  }

  function onkeydown(e: KeyboardEvent) {
    if (!card || e.repeat || Date.now() - shownAt < 300) return;
    // A focused field or button keeps its own Enter: a refusal the user tabbed to stays one.
    if (e.target instanceof Element && e.target.closest('input, select, textarea, button')) return;
    const n = /^[1-9]$/.test(e.key) ? Number(e.key) - 1 : -1;
    if (e.key === 'Escape') {
      e.preventDefault();
      answer(null);
    } else if (e.key === 'Enter' && card.default != null && card.choices[card.default]) {
      e.preventDefault();
      answer(card.choices[card.default]?.value);
    } else if (card.choices[n]) {
      e.preventDefault();
      answer(card.choices[n].value);
    }
  }

  onMount(() => {
    const offs = [
      on('connector-approval', async (view) => {
        if (s.section !== 'connectors') s.go('connectors');
        card = view;
        shownAt = Date.now();
        await tick();
        node?.scrollIntoView({ block: 'nearest' });
      }),
      on('connector-approval-closed', (id) => close(id)),
    ];
    return () => offs.forEach((p) => p.then((off) => off()));
  });
</script>

<svelte:document {onkeydown} />

{#snippet runs(list: Span[])}
  {#each list as r, i (i)}
    {#if r.kind === 'bold'}<strong>{r.text}</strong>{:else if r.kind === 'italic'}<em>{r.text}</em>{:else if r.kind === 'code'}<code
        >{r.text}</code
      >{:else if r.kind === 'link'}{r.text} ({r.url}){:else}{r.text}{/if}
  {/each}
{/snippet}

{#if card}
  <div id="approval-card" role="dialog" aria-labelledby="approval-title" bind:this={node}>
    <div class="who">{card.who}</div>
    <h2 id="approval-title">{card.title}</h2>
    <div class="message">
      {#each blocks as b, i (i)}
        {#if b.list}
          <ul>
            {#each b.items as item, j (j)}<li>{@render runs(item)}</li>{/each}
          </ul>
        {:else}
          <p>{@render runs(b.runs)}</p>
        {/if}
      {/each}
    </div>
    {#if card.note}<p class="lead" style="margin:var(--space-1) 0 0">{card.note}</p>{/if}
    <div class="keys">
      {#each card.choices as c, i (i)}
        <button class:main={i === card.default} onclick={() => answer(c.value)}><kbd>{i === card.default ? '↵' : String(i + 1)}</kbd>{c.label}</button>
      {/each}
      <button onclick={() => answer(null)}><kbd>Esc</kbd>Later</button>
    </div>
  </div>
{/if}

<style>
  #approval-card {
    border: 1px solid var(--color-accent); border-radius: var(--radius-card); padding: var(--space-3) var(--space-4); margin: 0 0 var(--space-4);
    background: var(--color-accent-soft);
  }
  .who { color: var(--color-label-secondary); font-size: var(--text-callout); }
  h2 { font-size: var(--text-title3); margin: var(--space-1) 0 var(--space-2); }
  .message p { margin: 0 0 var(--space-2); }
  .message ul { margin: 0 0 var(--space-2) var(--space-5); padding: 0; }
  .keys { display: flex; flex-wrap: wrap; gap: var(--space-2); margin-top: var(--space-3); align-items: center; }
  kbd {
    font: var(--text-subheadline) var(--font-mono); border: 1px solid var(--color-separator); border-radius: var(--radius-small);
    padding: 0 var(--space-1); margin-right: var(--space-1);
  }
</style>
