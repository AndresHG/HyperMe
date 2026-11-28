// Builds HyperMe's pages (specs/257-ui-framework, R-1). One entry per page; pages not yet moved to
// Svelte are copied as they are from app/ui (the public folder) until their slice.
import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { resolve } from 'node:path';

const page = (name: string) => resolve(import.meta.dirname, `${name}.html`);

export default defineConfig({
  plugins: [svelte()],
  // Relative paths: Tauri serves the pages from its own protocol, not from a web root.
  base: './',
  publicDir: '../ui',
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    // WebKit on macOS 13 and later (Safari 16.4 features), no older engine to support.
    target: 'safari16.4',
    sourcemap: false,
    rollupOptions: { input: { pill: page('pill') } },
  },
});
