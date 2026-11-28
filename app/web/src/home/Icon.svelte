<script lang="ts">
  // Home's icons, drawn inline (no icon font, no request), 16 by 16 on a 1.4 stroke unless sized.
  let { name, size = 16, stroke = 1.4 }: { name: string; size?: number; stroke?: number } = $props();
</script>

<svg aria-hidden="true" width={size} height={size} viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width={stroke}>
  {#if name === 'home'}<path d="M2.5 7 8 2.5 13.5 7v6.5h-11z" />
  {:else if name === 'history'}<circle cx="8" cy="8" r="6" /><path d="M8 4.8V8l2.2 1.6" />
  {:else if name === 'dictionary'}<path d="M3 2.5h8.5a1.5 1.5 0 0 1 1.5 1.5v9.5H4.5A1.5 1.5 0 0 1 3 12z" /><path
      d="M3 12a1.5 1.5 0 0 1 1.5-1.5H13"
    />
  {:else if name === 'settings'}<circle cx="8" cy="8" r="2.2" /><path
      d="M8 1.8v2M8 12.2v2M1.8 8h2M12.2 8h2M3.6 3.6l1.4 1.4M11 11l1.4 1.4M3.6 12.4 5 11M11 5l1.4-1.4"
    />
  {:else if name === 'help'}<circle cx="8" cy="8" r="6" /><path d="M6.3 6.2a1.8 1.8 0 1 1 2.4 1.7c-.5.2-.7.6-.7 1.1M8 11.2v.1" />
  {:else if name === 'report'}<path d="M3.5 14V2.5M3.5 3h8l-1.5 3 1.5 3h-8" />
  {:else if name === 'search'}<circle cx="7" cy="7" r="4.5" /><path d="m10.5 10.5 3 3" />
  {:else if name === 'copy'}<rect x="5" y="5" width="8.5" height="8.5" rx="1.5" /><path
      d="M11 5V3.5A1 1 0 0 0 10 2.5H3.5a1 1 0 0 0-1 1V10a1 1 0 0 0 1 1H5"
    />
  {:else if name === 'paste'}<path d="M13 3.5v5.5a2 2 0 0 1-2 2H3.5M6 8 3.5 11 6 14" />
  {:else if name === 'more'}<circle cx="3.5" cy="8" r=".6" /><circle cx="8" cy="8" r=".6" /><circle cx="12.5" cy="8" r=".6" />
  {:else if name === 'pencil'}<path d="M10.5 2.5l3 3-8 8H2.5v-3z" />
  {:else if name === 'trash'}<path d="M2.5 4.5h11M6 4.5V3h4v1.5M4 4.5l.7 9h6.6l.7-9" />
  {/if}
</svg>
