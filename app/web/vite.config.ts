// Builds HyperMe's pages (specs/257-ui-framework, R-1), one entry per page. Every page is Svelte;
// the tokens and shared controls' styles are imported by each page's main.ts.
import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { resolve } from 'node:path';

const page = (name: string) => resolve(import.meta.dirname, `${name}.html`);
const pages: Record<string, string> = { pill: page('pill'), onboarding: page('onboarding'), home: page('home') };
// The controls gallery, for the screenshot checks only (app/ui-tests): never in the app.
if (process.env.HYPERME_GALLERY) pages.gallery = page('gallery');

export default defineConfig({
  plugins: [svelte()],
  // Relative paths: Tauri serves the pages from its own protocol, not from a web root.
  base: './',
  publicDir: false,
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    // WebKit on macOS 13 and later (Safari 16.4 features), no older engine to support.
    target: 'safari16.4',
    sourcemap: false,
    rollupOptions: { input: pages },
  },
});
