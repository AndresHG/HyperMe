# HyperMe design system (macOS)

Required reading before any UI work. HyperMe must look and behave like a native Mac app, not a web
page in a window. The rules below come from Apple's Human Interface Guidelines (HIG) and the
system colours of macOS 14 and later. The values live in one file, `app/ui/tokens.css`; every
other file in `app/ui` uses them through `var(--…)`, and a test enforces it (see "Checks").

Owner decision, 27 November 2026: stay on Rust and Tauri, and adopt a strict macOS design system.
The "awkward on Mac" look and the square corners came from faking window chrome in CSS (a
transparent window with a painted card and shadow). This page exists so that does not happen again.

## Windows

- **Every normal window uses native chrome.** Decorations on, the native shadow on, and an overlay
  title bar with a hidden title (`TitleBarStyle::Overlay` and `hidden_title(true)`), so macOS
  draws the rounded corners, the shadow and the traffic lights. The page leaves
  `--title-bar-height` free at the top for the traffic lights and marks a drag region.
- **Only the pill may be transparent.** It is an `NSPanel` with no decorations; its shape is the
  capsule drawn by the page.
- **A page never draws window corners or a window shadow.** A sheet, popover or dialog is drawn
  inside a native window's content, never as its own transparent window. macOS draws a square
  shadow around a transparent window (the corner bug, hit twice in November 2026).
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
| `--text-headline` | 13 / 16 | semibold or bold | Emphasised body, row titles |
| `--text-body` | 13 / 16 | regular | Default text, controls |
| `--text-callout` | 12 / 15 | regular | Secondary text beside controls |
| `--text-subheadline` | 11 / 14 | regular | Meta lines, section labels |
| `--text-footnote` | 10 / 13 | regular | Footnotes |
| `--text-caption` | 10 / 13 | regular or medium | Captions, small badges |

Weights are `--weight-regular`, `--weight-medium`, `--weight-semibold` and `--weight-bold`.

## Spacing and size

- **4-point grid only:** `--space-1` 4, `--space-2` 8, `--space-3` 12, `--space-4` 16, `--space-5`
  20, `--space-6` 24, `--space-8` 32. Padding, margin and gap use these and `0`, nothing else.
- **Radii:** `--radius-small` 5 (small controls, tags), `--radius-control` 6 (buttons, fields,
  rows), `--radius-card` 10 (cards, popovers, menus), `--radius-sheet` 12 (sheets, the pill's
  cards), `--radius-capsule` 999 (the pill, the dot, toggles, badges).
- **Controls** follow Apple's control sizes: regular `--control-height` 22, small
  `--control-height-small` 18, large `--control-height-large` 28; horizontal padding
  `--control-padding-x` 8 (12 for large). These heights are frame heights read from Apple Design
  Resources for macOS; they are not yet checked against the current kit pixel by pixel
  (unverified). List rows are `--row-height` 28, the sidebar `--sidebar-width` 208.

## Colour

- Use the semantic tokens, never a raw colour: `--color-label`, `--color-label-secondary`,
  `--color-label-tertiary`, `--color-label-quaternary`, `--color-separator`, `--color-window-bg`,
  `--color-content-bg`, `--color-sidebar-bg`, `--color-card-bg`, `--color-control-bg`,
  `--color-control-border`, `--color-hover`, `--color-pressed`, `--color-selected`, `--color-fill`,
  `--color-scrim`, the system colours (`--color-red`, `--color-orange`, `--color-green`,
  `--color-yellow`, each with a `-text` variant that meets contrast on the content background) and
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
  the browser.
- **Selection:** interface text is not selectable (`user-select: none` on the body). Content is:
  transcripts, history items, dictionary entries, anything the user may want to copy.
- **Hover is minimal:** a faint `--color-hover` fill on rows and borderless buttons, nothing that
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
own look for a control. (The shared file arrives with the first migrated page.)

## Other platforms (later)

Linux gets `tokens-linux.css` from the GNOME HIG (Adwaita) and Windows `tokens-windows.css` from
Fluent 2, with the same token names. Components and pages stay the same; only the tokens file
changes. Window rules follow each platform's own chrome.

## Checks

- `cargo test -p hyperme-app --test design_tokens`: every file in the migrated list uses tokens
  for `border-radius`, `padding`, `margin`, `gap`, `font-size`, `font-family`, `font`, colours and
  shadows; only `0`, `auto` and CSS-wide keywords pass raw. A page joins the list when it moves to
  the tokens, so the check only ever grows.
