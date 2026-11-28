<script lang="ts" module>
  /** ok, bad, busy, or none. */
  export type Tone = 'ok' | 'bad' | 'busy' | '';
</script>

<script lang="ts">
  // A status: a dot and a few words, never colour alone.
  let { tone = '', text = '', id, large = false }: { tone?: Tone; text?: string; id?: string; large?: boolean } = $props();
</script>

<span {id} class="status {tone}" class:large>{text}</span>

<style>
  .status { display: inline-flex; align-items: center; gap: var(--space-2); font-size: var(--text-callout); font-weight: 500; white-space: nowrap; color: var(--color-label-secondary); }
  .status::before { content: ""; width: 7px; height: 7px; border-radius: var(--radius-capsule); background: var(--color-label-secondary); flex: none; }
  .status.ok { color: var(--color-green-text); } .status.ok::before { background: var(--color-green-text); }
  .status.bad { color: var(--color-red); } .status.bad::before { background: var(--color-red); }
  .status.busy::before { background: var(--color-accent); animation: pulse 900ms ease-in-out infinite; }
  .status:empty { display: none; }
  .status.large { font-size: var(--text-body); }
  @keyframes pulse { 50% { opacity: 0.25; } }
</style>
