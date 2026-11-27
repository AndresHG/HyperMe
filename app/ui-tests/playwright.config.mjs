// Page screenshot checks (docs/DESIGN.md, "Checks"): WebKit, the engine of macOS's web view, at
// fixed sizes, light and dark. Approved images live next to the spec; they are made on a Mac,
// because the system font and its rendering are part of what they show.
import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: '.',
  fullyParallel: true,
  reporter: 'list',
  snapshotPathTemplate: '{testDir}/approved/{arg}-{projectName}{ext}',
  expect: { toHaveScreenshot: { maxDiffPixelRatio: 0.002, animations: 'disabled' } },
  use: {
    baseURL: 'http://localhost:4173',
    browserName: 'webkit',
    deviceScaleFactor: 1,
    locale: 'en-GB',
    timezoneId: 'Europe/Madrid',
    trace: 'off',
  },
  projects: [
    { name: 'light', use: { colorScheme: 'light' } },
    { name: 'dark', use: { colorScheme: 'dark' } },
  ],
  webServer: { command: 'node serve.mjs', url: 'http://localhost:4173/tokens.css', reuseExistingServer: true },
});
