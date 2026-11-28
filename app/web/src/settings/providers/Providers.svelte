<script lang="ts">
  // Settings → AI models: Speech to text and Language model, one page per role. Places from the
  // app (`providers`, `providers/<role>` from the "keeps failing" notice, R-28) land here.
  import { sheet } from '../../home/settings.svelte';
  import { useSettings } from '../state.svelte';
  import ModelPage from './ModelPage.svelte';

  let { shown }: { shown: (id: string) => boolean } = $props();
  const s = useSettings();

  // A role named in the place scrolls its fallback, where a failing provider is replaced, into view.
  $effect(() => {
    void sheet.visit;
    const role = s.item;
    if (s.section !== 'speech' && s.section !== 'language') return;
    if (role === 'speech' || role === 'cleanup') {
      setTimeout(() => document.getElementById('fallback-' + role)?.scrollIntoView({ block: 'center' }), 0);
    }
  });
</script>

<ModelPage role="speech" id="speech" title="Speech to text" sub="The engine that turns your voice into words." on={shown('speech')} />
<ModelPage
  role="cleanup"
  id="language"
  title="Language model"
  sub="The model that cleans up your words."
  on={shown('language')}
/>
