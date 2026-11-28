<script lang="ts">
  import { Button, Card, CardRow, Field, Menu, Row, Segmented, Select, Sheet, Switch } from '../lib/ui';

  let on = $state(true);
  let off = $state(false);
  let text = $state('HyperMe');
  let empty = $state('');
  let notes = $state('Two lines\nof notes');
  let model = $state('whisper-large-v3-turbo');
  let keep = $state<number>(30);
  let selected = $state(1);
  let picked = $state('');
  const params = new URLSearchParams(location.search);
  let menu = $state(params.has('menu'));
  let sheet = $state(params.has('sheet'));
</script>

<main>
  <section>
    <h2>Button</h2>
    <div class="line">
      <Button>Default</Button>
      <Button variant="primary">Primary</Button>
      <Button variant="borderless">Borderless</Button>
      <Button disabled>Disabled</Button>
      <Button size="small">Small</Button>
      <Button size="large" variant="primary">Large</Button>
    </div>
  </section>

  <section>
    <h2>Field, select and switch</h2>
    <div class="line">
      <Field bind:value={text} aria-label="Name" />
      <Field bind:value={empty} placeholder="Search" type="search" aria-label="Search" />
      <Select
        bind:value={model}
        aria-label="Model"
        options={[
          { value: 'whisper-large-v3-turbo', label: 'Whisper large v3 turbo' },
          { value: 'whisper-large-v3', label: 'Whisper large v3' },
        ]}
      />
      <Switch bind:checked={on} aria-label="On" />
      <Switch bind:checked={off} aria-label="Off" />
    </div>
    <div class="line"><Field multiline bind:value={notes} rows={2} aria-label="Notes" /></div>
  </section>

  <section>
    <h2>Segmented</h2>
    <Segmented
      bind:value={keep}
      name="keep"
      label="Keep history"
      options={[
        { value: 7, label: '7 days' },
        { value: 30, label: '30 days' },
        { value: 0, label: 'Always' },
      ]}
    />
  </section>

  <section>
    <h2>Card</h2>
    <Card>
      <CardRow><span>Play sounds</span><Switch bind:checked={on} aria-label="Play sounds" /></CardRow>
      <CardRow><span>Language</span><span class="faint">Automatic</span></CardRow>
    </Card>
  </section>

  <section>
    <h2>Row</h2>
    <div role="listbox" aria-label="Rows">
      {#each ['Mail', 'Slack', 'Notes'] as app, i (app)}
        <Row role="option" aria-selected={i === selected} onclick={() => (selected = i)}>{app}</Row>
      {/each}
    </div>
  </section>
  <p class="faint" id="picked">{picked}</p>
</main>

<Menu
  bind:open={menu}
  x={320}
  y={360}
  items={[
    { id: 'copy', label: 'Copy', key: 'C' },
    { id: 'retry', label: 'Try again', key: 'R' },
    { id: 'delete', label: 'Delete', key: '⌫' },
  ]}
  onpick={(id) => (picked = id)}
/>
<Sheet bind:open={sheet} label="Example sheet">
  <div class="sheet-body">
    <h2>Sheet</h2>
    <Button variant="primary" onclick={() => (sheet = false)}>Done</Button>
  </div>
</Sheet>

<style>
  :global(body) { background: var(--color-content-bg); }
  main { padding: var(--space-6); display: grid; gap: var(--space-5); max-width: 720px; }
  h2 { margin: 0 0 var(--space-2); font: var(--weight-semibold) var(--text-headline) var(--font-system); }
  .line { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-2); margin-bottom: var(--space-2); }
  .faint { color: var(--color-label-secondary); }
  .sheet-body { padding: var(--space-6); }
</style>
