<script lang="ts">
  // Shortcuts: the dictation key, Fn, Right Option or a chord.
  import { useSettings } from './state.svelte';

  let { on }: { on: boolean } = $props();
  const s = useSettings();
  const PRESETS = ['fn', 'right_option'];
  const hotkey = $derived(s.view?.config?.hotkey ?? 'fn');
  // A chord picked but not typed yet shows its field before it is saved.
  let picked = $state<string | null>(null);
  const choice = $derived(picked ?? (PRESETS.includes(hotkey) ? hotkey : 'chord'));

  function choose(e: Event & { currentTarget: HTMLSelectElement }) {
    picked = e.currentTarget.value;
    if (picked !== 'chord') s.save('dictation', 'hotkey', picked);
  }
</script>

<section id="shortcuts" class:on>
  <h1>Shortcuts</h1>
  <p class="lead">Hold the dictation key while you speak. Esc cancels.</p>
  <div class="row" data-words="hotkey dictation fn globe option chord shortcut">
    <div class="what">Dictation (hold)<small>Fn is the Globe key. A chord looks like <code>ctrl+option+space</code>.</small></div>
    <span
      ><select value={choice} onchange={choose}>
        <option value="fn">Fn (Globe)</option>
        <option value="right_option">Right Option</option>
        <option value="chord">Chord…</option>
      </select>
      <input
        type="text"
        class:hidden={choice !== 'chord'}
        placeholder="ctrl+option+space"
        value={PRESETS.includes(hotkey) ? '' : hotkey}
        onchange={(e) => s.save('dictation', 'hotkey', e.currentTarget.value)}
      /></span
    >
  </div>
</section>
