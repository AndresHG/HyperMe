<script lang="ts">
  // The first screen: what the walkthrough will check.
</script>

<section class="screen" data-step="welcome">
  <div class="hero">
    <div class="mark" aria-hidden="true">
      <span>{#each [9, 16, 24, 16, 9] as h, i (i)}<i style:height="{h}px"></i>{/each}</span>
    </div>
    <h1>Welcome to HyperMe</h1>
    <p class="lead">Hold a key, speak, and your words appear where you type. Four quick checks and you are ready.</p>
    <ul class="steps-list">
      <li><span class="num">1</span><span><b>Microphone</b>, so HyperMe can hear you</span></li>
      <li><span class="num">2</span><span><b>Accessibility and Input Monitoring</b>, to paste and to see the key</span></li>
      <li><span class="num">3</span><span><b>Two keys</b>, for the speech and text services</span></li>
      <li><span class="num">4</span><span><b>A first dictation</b>, to see how fast it is</span></li>
    </ul>
  </div>
</section>

<style>
  .hero { padding-top: var(--space-6); }
  .mark {
    width: 56px; height: 56px; border-radius: var(--radius-sheet);
    background: var(--brand-mark);
    display: grid; place-items: center; margin-bottom: var(--space-6);
    box-shadow: var(--shadow-brand-mark);
  }
  .mark span { display: flex; gap: var(--space-1); align-items: center; }
  .mark i { display: block; width: 4px; border-radius: var(--radius-small); background: var(--color-on-accent); }
  .hero h1 { font-size: var(--text-large-title); }
  .steps-list { margin: var(--space-1) 0 0; padding: 0; list-style: none; display: grid; gap: var(--space-3); }
  .steps-list li { display: flex; gap: var(--space-3); align-items: center; color: var(--color-label-secondary); }
  .steps-list b { color: var(--color-label); font-weight: 500; }
  .num {
    width: 22px; height: 22px; border-radius: var(--radius-capsule); flex: none; display: grid; place-items: center;
    font-size: var(--text-subheadline); font-weight: 600; color: var(--color-accent); background: var(--color-accent-soft);
  }
</style>
