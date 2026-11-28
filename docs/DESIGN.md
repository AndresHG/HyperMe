# HyperMe design system (macOS)

Required reading before any UI work. HyperMe must look and behave like a native Mac app, not a web
page in a window. The rules below come from Apple's Human Interface Guidelines (HIG), the values
AppKit itself reports on macOS 26, and the write-ups listed under "Sources"; where a value is our
own convention rather than Apple's, it says so. The values live in one file, `app/ui/tokens.css`; every
other file in `app/ui` uses them through `var(--…)`, and a test enforces it (see "Checks").

Owner decision, 27 November 2026: stay on Rust and Tauri, and adopt a strict macOS design system.
The "awkward on Mac" look and the square corners came from faking window chrome in CSS (a
transparent window with a painted card and shadow). This page exists so that does not happen again.

## Windows

- **Every normal window uses native chrome.** Decorations on, the native shadow on, and an overlay
  title bar with a hidden title (`TitleBarStyle::Overlay` and `hidden_title(true)` in Tauri's
  window builder), so macOS draws the rounded corners, the shadow and the traffic lights, and the
  window shows its key and inactive states itself (the HIG asks custom chrome to reproduce them).
  The page leaves `--title-bar-height` free at the top for the traffic lights and marks a drag
  region with `data-tauri-drag-region`, which needs the `core:window:allow-start-dragging`
  permission and covers only the element that carries it, not its children.
- **Only the pill may be transparent.** It is an `NSPanel` with no decorations; its shape is the
  capsule drawn by the page. Its shadow is macOS's own (`has_shadow` and `invalidateShadow` after each
  resize), so it follows the capsule and the card; the 14-point bead has none, because macOS draws
  its outline in steps (docs/results/2026-11-28-pill-shadow.md).
- **A page never draws window corners or a window shadow.** A sheet, popover or dialog is drawn
  inside a native window's content, never as its own transparent window. A transparent window gets
  no correct system shadow (the shadow follows the window's shape only when `invalidateShadow` is
  called after it is drawn), and macOS drew a square one around ours: the corner bug, hit twice in
  November 2026. A child web view inside a rounded transparent window has the same problem (Tauri
  issue 14165, open).
- **Sheets** follow the HIG: a card with rounded corners over the dimmed window it belongs to, one
  at a time, with a sensible default size, closed by its button or Esc. Settings is such a sheet,
  inside the Home window's page.
- **No vibrancy in CSS.** Translucent materials come only from native materials
  (`NSVisualEffectView`), never `backdrop-filter`.
- A window opens where the user left it, at a sensible minimum size, and frees its web view when
  it closes (memory budgets in `CLAUDE.md`).

## Type

- Font: the system font only, `--font-system` (`-apple-system, BlinkMacSystemFont`, which is SF
  Pro). No web fonts, no `ui-rounded`, no SF Pro Rounded.
- Sizes are the macOS text styles. Use the name for its role, not the size you want.

| Token | Size / line | Weight | Use |
| --- | --- | --- | --- |
| `--text-large-title` | 26 / 32 | bold | The one title of a page (Home's section titles) |
| `--text-title1` | 22 / 26 | regular or bold | Onboarding step titles |
| `--text-title2` | 17 / 22 | regular or semibold | Sheet and card titles |
| `--text-title3` | 15 / 20 | regular or semibold | Group titles inside a page |
| `--text-headline` | 13 / 16 | bold | Emphasised body, row titles |
| `--text-body` | 13 / 16 | regular | Default text, controls |
| `--text-callout` | 12 / 15 | regular | Secondary text beside controls |
| `--text-subheadline` | 11 / 14 | regular | Meta lines, section labels |
| `--text-footnote` | 10 / 13 | regular | Footnotes |
| `--text-caption` | 10 / 13 | regular (Caption 1) or medium (Caption 2) | Captions, small badges |

Weights are `--weight-regular`, `--weight-medium`, `--weight-semibold` and `--weight-bold`.

## Spacing and size

- **4-point grid only** (a house rule: Apple publishes no spacing grid or window margin for
  macOS): `--space-1` 4, `--space-2` 8, `--space-3` 12, `--space-4` 16, `--space-5`
  20, `--space-6` 24, `--space-8` 32. Padding, margin and gap use these and `0`, nothing else.
- **Radii:** `--radius-small` 5 (small controls, tags), `--radius-control` 6 (buttons, fields,
  rows), `--radius-card` 10 (cards, popovers, menus), `--radius-sheet` 12 (sheets, the pill's
  cards), `--radius-capsule` 999 (the pill, the dot, toggles, badges). Apple gives no figure for
  control or sheet radii; these are ours, chosen to sit inside macOS 26's window corners (about 16
  for a window with a title bar only, larger with a toolbar; unofficial, see Sources).
- **Controls** follow the frame heights AppKit gives push buttons and pop-up buttons on macOS 26
  (measured with `fittingSize` on 27 November 2026; the HIG gives no macOS heights): regular
  `--control-height` 24, small `--control-height-small` 20, mini `--control-height-mini` 16, large
  `--control-height-large` 28. Text fields are 24 regular. Horizontal padding is
  `--control-padding-x` 8 (12 for large). List rows are `--row-height` 28, the sidebar
  `--sidebar-width` 208. Older macOS versions drew regular controls at 22; we follow macOS 26.

## Colour

- Use the semantic tokens, never a raw colour. Their values are what AppKit's `NSColor` reports on
  macOS 26 (the HIG lists the names, not values; the system colours changed on 9 June 2025): `--color-label`, `--color-label-secondary`,
  `--color-label-tertiary`, `--color-label-quaternary`, `--color-separator`, `--color-window-bg`,
  `--color-content-bg`, `--color-sidebar-bg`, `--color-card-bg`, `--color-control-bg`,
  `--color-control-border`, `--color-hover`, `--color-pressed`, `--color-selected`, `--color-fill`,
  `--color-scrim`, the system colours (`--color-red`, `--color-orange`, `--color-green`,
  `--color-yellow`, `--color-blue`, each with a `-text` variant that meets contrast on the content background) and
  the shadows `--shadow-card`, `--shadow-popover`, `--shadow-sheet`.
- The accent is Hyper violet: `--color-accent` (focus rings, selection, the main button),
  `--color-accent-text` (violet text), `--color-accent-soft` and `--color-accent-selected`
  (selected rows), `--color-on-accent` (text on the accent).
- Light and dark come from `prefers-color-scheme`; every token has both values in `tokens.css`.
- The pill and its cards stay dark in both appearances: `--color-pill-*`, with the state colours
  `--color-recording`, `--color-done` and `--color-raw` (docs/UX.md, "Visual language").
- Colour is never the only signal: a state also has an icon or a word. Text meets WCAG AA.
- Honour `prefers-reduced-transparency` (the scrim and hovers turn opaque; `tokens.css` does it)
  and `prefers-reduced-motion` (all durations become 0; motion uses `--duration` and `--easing`).

## Behaviour

- **Cursor:** the default arrow everywhere, buttons included. The hand only for a link that opens
  the browser (Evil Martians; Raycast does the same).
- **Selection:** interface text is not selectable (`user-select: none` on the body). Content is:
  transcripts, history items, dictionary entries, anything the user may want to copy.
- **Context menus** where a Mac app has them (rows of a list: copy, delete).
- **Hover is minimal** (Evil Martians): a faint `--color-hover` fill on rows and borderless buttons, nothing that
  moves. **Pressed** (`--color-pressed`) and **selected** (`--color-accent-selected`, or
  `--color-selected` when the window is not key) are always clear.
- **Scrolling:** the page itself never bounces or scrolls (`overscroll-behavior: none`, `overflow:
  hidden` on the body); only content areas scroll.
- **Focus:** keyboard focus shows a ring in the accent (`--focus-ring-width` of
  `--color-focus-ring`), only for keyboard focus (`:focus-visible`).
- **Keyboard:** everything is reachable by Tab; Enter confirms the default button, Esc closes a
  sheet, popover or card; arrows move in lists and segmented controls; `⌘,` opens Settings.
- **Motion:** short, from the element's own position, never decorative.

## Components

Built only from tokens, in one shared CSS file, the same on every page: button (default, primary,
borderless), field, toggle, segmented control, list row, card, menu. A page adds layout, never its
own look for a control. The file is `app/ui/components.css`; every page links `tokens.css`, then
`components.css`. Its classes carry a `ui-` prefix (`ui-button primary`, `ui-field`,
`ui-switch`, `ui-segmented`, `ui-row`, `ui-card`, `ui-menu`), and it holds the behaviour every page
shares (arrow cursor, non-selectable interface text, focus ring, no page bounce).

Pages written in Svelte (spec 257) use the same controls as components in `app/web/src/lib/ui/`:
`Button`, `Field`, `Select`, `Switch`, `Segmented`, `Row`, `Card` with `CardRow`, `Menu` and
`Sheet`, each with its own styles from the tokens. `app/web/gallery.html` shows every one in its
states; the screenshot checks build it (`HYPERME_GALLERY=1`), the app never does. When the last
page leaves `app/ui`, `components.css` keeps only the shared behaviour.

## Other platforms (later)

Linux gets `tokens-linux.css` from the GNOME HIG (Adwaita) and Windows `tokens-windows.css` from
Fluent 2, with the same token names. Components and pages stay the same; only the tokens file
changes. Window rules follow each platform's own chrome.

## Sources

Read on 27 November 2026.

- Apple HIG: [Typography](https://developer.apple.com/design/human-interface-guidelines/typography)
  (macOS text styles, sizes, line heights and weights),
  [Color](https://developer.apple.com/design/human-interface-guidelines/color) (semantic colour
  names; updated system colours, 9 June 2025),
  [Layout](https://developer.apple.com/design/human-interface-guidelines/layout) (no macOS grid or
  margin given), [Windows](https://developer.apple.com/design/human-interface-guidelines/windows),
  [Sheets](https://developer.apple.com/design/human-interface-guidelines/sheets),
  [Materials](https://developer.apple.com/design/human-interface-guidelines/materials),
  [Buttons](https://developer.apple.com/design/human-interface-guidelines/buttons) (no macOS
  heights or radius given).
- Colour values and control heights: read from AppKit (`NSColor`, `NSButton.fittingSize`,
  `NSPopUpButton`, `NSTextField`) on macOS 26.6 by `scripts/macos-system-values.swift`, 27 November
  2026. Run it again on a new macOS and update `tokens.css`.
- [NSWindow invalidateShadow()](https://developer.apple.com/documentation/appkit/nswindow/invalidateshadow()):
  the shadow is recomputed from the window's current shape when asked.
- Tauri 2: [Window customization](https://v2.tauri.app/learn/window-customization/) (drag regions,
  window permissions), the `TitleBarStyle` and `WebviewWindowBuilder` API on docs.rs,
  [issue 14165](https://github.com/tauri-apps/tauri/issues/14165) (child web view in a rounded
  transparent window draws square corners; open).
- [window-vibrancy](https://docs.rs/window-vibrancy) 0.8: `apply_vibrancy` with an
  `NSVisualEffectMaterial` and a corner radius, and `apply_liquid_glass` on macOS 26.
- Evil Martians, [How to make absolutely any app look like a macOS app](https://evilmartians.com/chronicles/how-to-make-absolutely-any-app-look-like-a-macos-app)
  (16 June 2022): default cursor, non-selectable interface text, no hover on controls, sparing
  accent colour, context menus.
- Raycast, [A technical deep dive into the new Raycast](https://www.raycast.com/blog/a-technical-deep-dive-into-the-new-raycast)
  (14 May 2026): web views on a native window background, native windows for popovers and
  tooltips, Settings as its own native window, no pointer cursor, little hover.
- macOS 26 window corners vary by window type, about 16 pt with a title bar only (unofficial:
  [Mozilla bug 2053333](https://bugzilla.mozilla.org/show_bug.cgi?id=2053333),
  [Michael Tsai, 16 October 2025](https://mjtsai.com/blog/2025/10/16/tahoe-window-corners/)).

## Checks

- `cargo test -p hyperme-app --test design_tokens`: every Svelte component in `app/web/src` and
  every file in the migrated list uses tokens
  for `border-radius`, `padding`, `margin`, `gap`, `font-size`, `font-family`, `font`, colours and
  shadows; only `0`, `auto` and CSS-wide keywords pass raw. A page joins the list when it moves to
  the tokens, so the check only ever grows.
- `cd app/ui-tests && npm install && npm test` (about 10 s): builds the pages (`app/web`), then
  shows every page in WebKit (the engine of
  macOS's web view) at a fixed size, light and dark, with fixture data from `mock-tauri.js` in place
  of Tauri's API, compared with the approved images in `app/ui-tests/approved/`. Home, History,
  Dictionary, the Settings sheet, the walkthrough and the pill (its card and a state): twelve images.
  Each page also gets a layout check: no text runs past its box unless an ellipsis cuts it, and no
  two pieces of text overlap.
- After a change you mean to make: `npm run update`, look at every changed image in
  `app/ui-tests/approved/` (git shows them side by side), and commit them with the change. Make
  them on a Mac: the system font is part of the picture, so images from another system differ.
- A page that needs new data adds it to `mock-tauri.js`: synthetic data only, never a real
  dictation.

