// Settings is a sheet in Home's page (V0.1 redesign), a component whose code loads the first time
// it opens (spec 257, R-10) and which is dropped when it closes, so it costs nothing while closed.
// The sidebar, ⌘, and the app open it; Rust calls `window.openSettings` (settings::open).
import { commands } from '../lib/ipc';

/**
 * Whether the sheet is open, and the place it shows: a section (`privacy`), or a section and an
 * item (`connectors/google:work`, `diagnostics/report/12`). `visit` counts each opening, so asking
 * for the place already shown still goes there (a section always opens at its top).
 */
export const sheet = $state({ open: false, place: 'setup', visit: 0 });

export function openSettings(place: string) {
  sheet.place = place;
  sheet.visit++;
  if (sheet.open) return;
  sheet.open = true;
  commands.settingsOpened();
}

export function closeSettings() {
  if (!sheet.open) return;
  sheet.open = false;
  commands.settingsClosed();
}
