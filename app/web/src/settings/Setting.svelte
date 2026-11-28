<script lang="ts">
  // One config value as a switch, a number or a text field, saved when it changes (R-24).
  import type { ConfigView } from '../lib/ipc';
  import { useSettings } from './state.svelte';

  let {
    section,
    key,
    field,
    type = 'checkbox',
    min,
    max,
  }: { section: string; key: string; field: keyof ConfigView; type?: 'checkbox' | 'number' | 'text'; min?: number; max?: number } =
    $props();
  const s = useSettings();
  const value = $derived(s.view?.config?.[field]);

  function onchange(e: Event & { currentTarget: HTMLInputElement }) {
    const input = e.currentTarget;
    s.save(section, key, type === 'checkbox' ? input.checked : type === 'number' ? Number(input.value) : input.value);
  }
</script>

{#if type === 'checkbox'}
  <input type="checkbox" checked={!!value} {onchange} />
{:else}
  <input {type} {min} {max} value={value ?? ''} {onchange} />
{/if}
