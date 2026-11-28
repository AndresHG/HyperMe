<script lang="ts">
  // Today (#92): tasks on the left in Things' style, the schedule on the right. Rolled over too
  // often, a task asks once below the list (R-14); tasks waiting on another sit under it (R-10).
  import { commands, type TaskAction, type TodayPage } from '../lib/ipc';
  import { duration } from './format';
  import TaskRow from './TaskRow.svelte';

  let page = $state<TodayPage | null>(null);
  let error = $state('');
  let failed = $state('');
  let draft = $state('');
  let adder = $state<HTMLInputElement>();

  /** Reads the day again. */
  export async function load() {
    try {
      page = await commands.homeToday();
      failed = '';
    } catch (e) {
      failed = String(e);
    }
  }

  async function act(id: number, action: TaskAction, value: string | null = null) {
    try {
      await commands.tasksAct(id, action, value);
    } catch (e) {
      error = String(e);
    }
    await load();
  }

  async function add(e: KeyboardEvent) {
    if (e.key !== 'Enter' || !draft.trim()) return;
    e.preventDefault();
    try {
      await commands.tasksAdd(draft);
      draft = '';
    } catch (err) {
      error = String(err);
    }
    await load();
    adder?.focus();
  }

  const left = $derived(page?.lines.length ?? 0);
  const next = $derived(page?.meetings.find((m) => m.now) ?? page?.meetings.find((m) => !m.past));

</script>

<div id="today-root">
  {#if failed}
    <p class="error">{failed}</p>
  {:else if page}
    {#if error}<p class="error">{error}</p>{/if}
    <div class="today">
      <div>
        <h2 class="tasks-title">Today · {page.done_today ? page.done_today + ' done, ' : ''}{left === 1 ? '1 to go' : left + ' to go'}</h2>
        <div class="tasks">
          {#each page.lines as line (line.id)}<TaskRow {line} act={(action, value) => act(line.id, action, value)} />{/each}
          <div class="adder">
            <span>+</span>
            <input class="add" bind:this={adder} bind:value={draft} placeholder="New task" aria-label="New task for today" onkeydown={add} />
          </div>
        </div>
        {#each page.lines.filter((l) => l.stale) as line (line.id)}
          <div class="stale">
            <span class="what"><b>{line.title}</b> has rolled over {line.rolled} times.</span>
            <button onclick={() => act(line.id, 'keep')}>Keep</button>
            <input
              type="date"
              min={page.tomorrow}
              aria-label="Pick a day for {line.title}"
              onchange={(e) => e.currentTarget.value && act(line.id, 'move', e.currentTarget.value)}
            />
            <button onclick={() => act(line.id, 'drop')}>Drop</button>
          </div>
        {/each}
        {#if page.waiting.length}
          <div class="waiting">
            <h2>Waiting on · {page.waiting.length}</h2>
            {#each page.waiting as w (w.id)}
              <div class="wait">
                <span class="title">{w.title}</span>
                {#if w.new}<span class="tag new">new</span>{/if}
                <span class="tag">after {w.on}</span>
              </div>
            {/each}
          </div>
        {/if}
      </div>
      <aside>
        <h2>Schedule</h2>
        <div class="meets">
          {#each page.meetings as m, i (i)}
            <div class="meet" class:past={m.past} class:next={m === next}>
              <time>{m.start}</time>
              <span class="title">
                <b>{m.title}</b>
                {#if m.now}<span class="soon">now · until {m.end}</span>
                {:else if m.starts_in_min != null}<span class="soon">in {duration(m.starts_in_min * 60)}</span>{/if}
              </span>
              {#if m.link && !m.past}
                <button onclick={() => commands.tasksOpenMeeting(m.link ?? '').catch((e) => (error = String(e)))}>Join</button>
              {/if}
            </div>
          {:else}
            <p class="quiet">No meetings today.</p>
          {/each}
        </div>
      </aside>
    </div>
  {/if}
</div>

<style>
  .today { display: grid; grid-template-columns: minmax(0, 1fr) 280px; gap: var(--space-8); align-items: start; }
  .tasks-title { margin-left: var(--space-2); }
  .tasks { display: flex; flex-direction: column; gap: var(--space-1); }
  .adder { display: flex; align-items: center; gap: var(--space-3); padding: var(--space-2); border-radius: var(--radius-control); min-height: 38px; color: var(--color-label-secondary); }
  .adder:hover { background: var(--color-hover); }
  .adder span { width: 18px; text-align: center; font-size: var(--text-title3); line-height: 1; }
  .adder input { font: inherit; padding: var(--space-1) 0; border-radius: var(--radius-control); border: 0; background: transparent; color: inherit; flex: 1; outline: none; }
  .stale { display: flex; align-items: center; gap: var(--space-2); margin: var(--space-4) var(--space-2) 0; padding: var(--space-3) var(--space-3); border-radius: var(--radius-card); border: 1px dashed var(--color-separator); color: var(--color-label-secondary); }
  .stale .what { flex: 1; min-width: 0; }
  .stale > button, .stale > input { flex: none; }
  .stale b { color: var(--color-label); }
  .stale button { font-size: var(--text-callout); padding: var(--space-1) var(--space-2); border-radius: var(--radius-control); border: 1px solid var(--color-separator); background: var(--color-content-bg); }
  .stale button:hover { border-color: var(--color-accent); }
  .stale input { font: inherit; font-size: var(--text-callout); padding: var(--space-1) var(--space-2); border-radius: var(--radius-control); border: 1px solid var(--color-separator); background: var(--color-content-bg); color: inherit; }
  .waiting { margin-top: var(--space-6); }
  .waiting h2 { margin-left: var(--space-2); }
  .wait { display: flex; align-items: center; gap: var(--space-3); padding: var(--space-2); color: var(--color-label-secondary); }
  .wait .title { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .meets { display: flex; flex-direction: column; gap: var(--space-2); }
  .meet { display: flex; gap: var(--space-3); align-items: center; padding: var(--space-2) var(--space-3); border-radius: var(--radius-card); }
  .meet time { width: 42px; flex: none; font-variant-numeric: tabular-nums; color: var(--color-label-secondary); }
  .meet .title { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: var(--space-1); }
  .meet .title b { font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .meet .soon { font-size: var(--text-callout); color: var(--color-accent-text); font-weight: 600; }
  .meet.past { color: var(--color-label-secondary); }
  .meet.past time { color: var(--color-label-secondary); }
  .meet.next { background: var(--color-accent-soft); }
  .meet.next time, .meet.next .title b { font-weight: 600; color: var(--color-label); }
  .meet button { padding: var(--space-1) var(--space-3); border-radius: var(--radius-control); border: 0; background: var(--color-accent); color: var(--color-on-accent); font-size: var(--text-callout); font-weight: 600; }
  .quiet { color: var(--color-label-secondary); padding: var(--space-2) var(--space-3); }
</style>
