// Settings is a sheet in Home's page (V0.1 redesign): settings.html in a frame, made when it opens
// and dropped when it closes, so it costs no memory while closed. The sidebar, ⌘, and the app open
// it; Rust calls `window.openSettings` (settings::open), and the frame calls `closeSettings`.
// Slice 6 of spec 257 replaces the frame with a component.
import { commands } from '../lib/ipc';

export const sheet = $state<{ src: string | null; frame: HTMLIFrameElement | null }>({ src: null, frame: null });

export function openSettings(section: string) {
  const frame = sheet.frame;
  if (sheet.src !== null && frame?.contentWindow) {
    frame.contentWindow.location.hash = section;
    frame.focus();
    return;
  }
  sheet.src = 'settings.html#' + section;
  commands.settingsOpened();
}

export function closeSettings() {
  if (sheet.src === null) return;
  try {
    (sheet.frame?.contentWindow as (Window & { teardown?: () => void }) | null)?.teardown?.();
  } catch {
    // Already gone.
  }
  sheet.src = null;
  sheet.frame = null;
  commands.settingsClosed();
}
