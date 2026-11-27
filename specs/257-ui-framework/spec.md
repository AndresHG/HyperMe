# UI framework: Svelte 5, TypeScript and Vite

**Status:** draft, 28 November 2026.
**Issue:** [#257](https://github.com/AndresHG/hyperme/issues/257). **Size:** L (a new build step
and toolchain, every page, more than one pull request).
**Traces to:** the audit of 27 November 2026 (project thread "Audit and refactor the frontend and
backend"). The owner chose this direction on 27 November 2026: "I prefer using proper libraries
that are state of the art over building from scratch ourselves to save 1-4 MB". Framework picked
by Claude at the owner's request ("whatever is state of the art or the best practice").
**Replaces:** the decision "a thin web UI" in `CLAUDE.md` keeps its meaning (the Rust core does the
work), but the UI gets a framework and a build step.

## Problem

`app/ui` is about 2,800 lines of plain JavaScript that builds the DOM by hand, in six pages:

- No components. The same element helper exists in five versions with three signatures; buttons,
  switches, rows and menus are rebuilt per page. `components.css` holds shared styles, but each
  page still writes its own markup for them.
- No types. Every Rust command answers JSON that the page reads by field name; a renamed field in
  Rust breaks a page silently, and only the screenshot checks or the owner notice.
- Large inline scripts (`home.html` 950 lines, `settings.html` 550) mix routing, state, drawing
  and command calls, and cannot be tested in parts.
- State lives in module variables and is redrawn by hand (`refresh()`, `redrawToday()`), so a
  change in one place easily leaves another stale.
- Settings runs in an iframe inside Home and borrows Home's Tauri API through a wrapper. The frame
  is a second document that costs about 26 MB (docs/results/2026-11-27-settings-sheet-memory.md),
  which puts Home with Settings open at 137 MB, over the 130 MB target.

## Goal

The UI is written as Svelte 5 components in TypeScript, built by Vite into the files Tauri serves.
Commands and their answers are typed from the Rust side, so a mismatch fails the build. Shared
controls (button, switch, field, segmented control, card, menu, sheet) are components built only
from `tokens.css`. Every page looks and behaves as it does today, and the memory budgets hold.

## Words used here

- **Page:** one window's document: pill, walkthrough (onboarding), Home. Settings stops being a
  page and becomes a component inside Home.
- **Bindings:** the TypeScript file generated from the Rust commands and their types.

## Requirements

### Build

- R-1. HyperMe shall build its UI with Vite from `app/web/`, one entry per page, into a folder
  Tauri serves (`frontendDist`).
- R-2. When a developer runs `cargo build` or `cargo test` for `hyperme-app`, the build shall
  build the UI first when its sources changed, so the existing commands and scripts
  (`scripts/bundle-macos.sh`, `scripts/release-macos.sh`) keep working unchanged.
- R-3. When a developer runs the app in development mode, the pages shall load from Vite's dev
  server with hot reload (`devUrl`).
- R-4. The UI dependencies shall be MIT, Apache-2.0 or BSD, pinned in `package-lock.json`.
- R-5. A release build shall contain only the built files: no source maps, no dev server code.

### Code

- R-6. Pages and components shall be written in TypeScript with strict checks; `svelte-check`
  shall pass with no errors or warnings.
- R-7. The shared controls shall be Svelte components in `app/web/src/lib/ui/`, styled only with
  tokens from `tokens.css` (docs/DESIGN.md). A page shall not restyle a control.
- R-8. Every call to a Rust command and every event shall go through the generated bindings; no
  page shall call `invoke` with a command name in a string.
- R-9. The bindings shall be generated from the Rust commands (tauri-specta) and checked in; a
  test shall fail when they are out of date.
- R-10. Settings shall be a component loaded on demand inside Home's document, not an iframe; its
  code shall not load until Settings first opens.
- R-11. Page state shall live in Svelte runes (`$state`, `$derived`) or small stores per feature,
  not in hand-written redraw functions.

### Behaviour and budgets

- R-12. Each migrated page shall match its approved screenshots (app/ui-tests) in light and dark,
  with no layout problems, unless a change is approved with it.
- R-13. Idle memory shall stay under 100 MB, and within 3 MB of the last measurement (60.4 MB).
- R-14. Home open shall stay at 130 MB or less, and Home with Settings open shall be at 130 MB or
  less.
- R-15. Key-down to recording and key release to cleaned text shall not change: the pill page
  shall show its first frame as fast as today (measured, within 5 ms).

## Acceptance criteria

- AC-1 (R-1, R-2, R-5). On a clean checkout with Node installed, `cargo build --release -p
  hyperme-app` builds the UI and the app; the app bundle holds no `.map` file and no Vite client.
- AC-2 (R-3). `npm run tauri dev` in `app/web` opens the app with hot reload of a page.
- AC-3 (R-6, R-7). `npm run check` (svelte-check, ESLint) passes; the token lint covers the
  `<style>` blocks of `.svelte` files.
- AC-4 (R-8, R-9). No `invoke('` string remains in `app/web/src` outside the generated bindings;
  the bindings test fails when a command's argument is renamed in Rust and the file is not
  regenerated.
- AC-5 (R-10, R-14). Opening Settings from Home loads its chunk then; Home with Settings open
  measures 130 MB or less on the owner's Mac (release build, `footprint -p`).
- AC-6 (R-12). `npm test` in `app/ui-tests` passes against the built pages with the approved
  images unchanged, except where a pull request says otherwise.
- AC-7 (R-13, R-15). Idle memory and pill first-frame time measured after each page's pull request
  and written to `docs/results/`.

## Non-goals

- No visual redesign. The pages keep today's look; the design thread owns changes to it.
- No SvelteKit, router library or server-side rendering: these are three small desktop windows.
- No CSS framework (Tailwind and similar): `tokens.css` and component styles are the system.
- No change to what leaves the machine.

## Pre-mortem

- *The build broke for contributors without Node.* The README and CONTRIBUTING name Node as a
  prerequisite, and `build.rs` fails with a clear message when `npm` is missing (R-2).
- *A page drifted from its look during the move.* The screenshot checks gate each page (R-12).
- *Memory grew a little with each page until Home passed 130 MB.* Measured per pull request (AC-7),
  and Settings leaves its iframe, which frees more than the framework costs (R-10).
- *tauri-specta, still a 2.0 release candidate, broke on an update.* Its version is pinned
  exactly, and the bindings are checked in, so an update is a deliberate change with a diff.
- *Two UIs lived side by side for weeks.* The move goes page by page in consecutive pull requests,
  and unmigrated pages are served as they are from the same build until their turn.

## Decisions

- **Svelte 5** over Preact, React and Lit: it compiles components to plain DOM updates with no
  virtual DOM and the smallest runtime among the major frameworks, it has first-class TypeScript,
  and it is one of Tauri's official templates (Claude, at the owner's request, 28 November 2026).
- **Vite** as the build tool: the standard for Svelte and Tauri, with a dev server and hot reload.
- **tauri-specta** for the bindings, over hand-written types or ts-rs: it generates both the types
  and the typed command functions from the Rust signatures. Pinned exactly while it is a release
  candidate.
- **Keep the screenshot checks** (app/ui-tests) as the guard for the move, pointed at the built
  pages.
- **build.rs runs the UI build**, rather than requiring the Tauri CLI for every build, so the
  existing `cargo` commands and scripts stay as they are. The Tauri CLI is used for dev mode.
