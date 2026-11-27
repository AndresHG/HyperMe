<script lang="ts">
  // The card the pill grows into: the morning card (#92, docs/ux/daily-cards.svg) or an alert
  // (docs/UX.md, "Alerts"). Heights match pill.rs (CARD_*), so the panel fits it exactly. It
  // never takes focus, so it is answered with a click.
  import { commands, type Card } from '../lib/ipc';

  let { card }: { card: Card | null } = $props();
</script>

<div id="card" class="card" role="dialog" aria-label="Morning card" hidden={!card}>
  {#if card?.kind === 'alert'}
    <h1><span class="bad">!</span> {card.title}</h1>
    <div class="line strong">{card.detail}</div>
    <div class="keys">
      <button class="main" onclick={() => commands.cardAction()}>{card.action}</button>
      <button onclick={() => commands.cardDismiss()}>Later</button>
    </div>
  {:else if card}
    <h1>{card.title}</h1>
    {#if card.yesterday}<div class="line">{card.yesterday}</div>{/if}
    {#if card.meetings}<div class="line strong">{card.meetings}</div>{/if}
    {#each card.tasks as t, i (i)}
      <div class="task {t.tone ?? ''}">
        <i></i><span>{t.title}</span>
        {#if t.mark}<em>{t.mark}</em>{/if}
      </div>
    {/each}
    {#if card.footer}<div class="line">{card.footer}</div>{/if}
    {#if card.mail}<div class="line strong">{card.mail}</div>{/if}
    {#if card.stale}<div class="line">{card.stale}</div>{/if}
    <div class="keys">
      <button class="main" onclick={() => commands.cardOpenToday()}>Open Today</button>
      <button onclick={() => commands.cardDismiss()}>Later</button>
    </div>
  {/if}
</div>

<style>
  .card {
    width: 100%; height: 100%; padding: var(--space-5); border-radius: var(--radius-sheet);
    background: var(--color-pill-bg); box-shadow: inset 0 0 0 1px var(--color-pill-border-strong); color: var(--color-pill-label);
    font: var(--text-callout) var(--font-system); overflow: hidden;
  }
  .card[hidden] { display: none; }
  h1 { margin: 0; height: 24px; font: 700 var(--text-title3) var(--font-system); }
  .line { height: 20px; line-height: 20px; color: var(--color-pill-label-secondary); font-size: var(--text-subheadline); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .line.strong { color: var(--color-pill-label); font-size: var(--text-callout); }
  .task { height: 28px; margin-bottom: var(--space-1); border-radius: var(--radius-card); background: var(--color-pill-raised); display: flex; align-items: center; gap: var(--space-2); padding: 0 var(--space-3); }
  .task i { width: 7px; height: 7px; border-radius: var(--radius-capsule); box-shadow: inset 0 0 0 1px var(--color-pill-label-secondary); flex: none; }
  .task.due i { background: var(--color-raw); box-shadow: none; }
  .task.unblocked i { background: var(--color-accent); box-shadow: none; }
  .task span { flex: 1; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .task em { font-style: normal; font-size: var(--text-subheadline); color: var(--color-pill-label-tertiary); }
  .task.due em { color: var(--color-raw); }
  .task.unblocked em { color: var(--color-pill-accent-text); }
  .keys { height: 42px; display: flex; align-items: flex-end; gap: var(--space-2); }
  button { font: 600 var(--text-subheadline) var(--font-system); height: 26px; padding: 0 var(--space-3); border-radius: var(--radius-control); border: 0; background: var(--color-pill-hover); color: var(--color-pill-label-secondary); }
  button.main { background: var(--color-accent); color: var(--color-on-accent); }
  .bad { color: var(--color-recording); }
</style>
