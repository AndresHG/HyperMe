# Home window memory (spec 98, AC-2), 26 November 2026

The Home window from slices 3 and 4 on a history of 1,000 made-up items (synthetic words, 100 of
them with a 0.5 s silent WAV, 20 failed), in a scratch home folder with no keychain access, so no
dictation ran. Release build, run from the command line (not a signed bundle). Home was opened,
moved and closed by a temporary, uncommitted build that read commands from a file; the committed
code has no such hook.

- Machine: MacBook Pro (MacBookPro18,3), Apple M1 Pro, macOS 26.6.2
- Build: `feat/98-home-actions` at 08c3abd plus this commit's changes, release
- Tools: `footprint -p <pid>` (physical footprint, what Activity Monitor shows as Memory) for
  `hyperme` and every WebKit process that started with it; CPU from `top -l 7 -s 2`; the app's
  windows from `CGWindowListCopyWindowInfo`
- Network: not involved

## Result (final run)

| Step | `hyperme` | WebKit GPU | WebKit Networking | Pill WebContent | Home WebContent | **All** |
| --- | --- | --- | --- | --- | --- | --- |
| 1. Idle, Home never opened, 30 s | 29 MB | 11 MB | 6.5 MB | 13 MB | none | **59.5 MB** |
| 2a. Home open on the Home page, 10 s | 37 MB | 15 MB | 6.4 MB | 13 MB | 42 MB | **113.4 MB** |
| 2b. History, scrolled until 1,128 rows were loaded | 39 MB | 19 MB | 6.4 MB | 13 MB | 90 MB | **167.4 MB** |
| 3. 30 s after Home closed | 38 MB | 17 MB | 6.3 MB | 13 MB | gone | **74.3 MB** |
| 4. Five more open and close cycles, then 30 s idle | 36 MB | 17 MB | 6.4 MB | 13 MB | gone | **72.4 MB** |

Idle CPU of all processes after Home closed: 0.00% over 12 seconds.

## Verdict per target

| Target (AC-2) | Measured | Verdict |
| --- | --- | --- |
| Home open on 1,000 items: under 100 MB | 113.4 MB on the Home page; 167.4 MB on History after scrolling through 1,128 rows | **Fails** |
| 30 s after close: within 5 MB of step 1 | 74.3 MB against 59.5 MB: 14.8 MB above | **Fails** |
| Home's WebContent process gone after close | Gone in every run | Passes |
| Five open and close cycles grow it by no more than 5 MB | 72.4 MB after five cycles against 74.3 MB after the first: no growth | Passes |
| Idle CPU after close near 0% | 0.00% | Passes |

## Where the memory goes

- **Home's page process.** 42 MB on the Home page, of which 20 MB is graphics (the page's layer
  and tile surfaces) and 14 MB WebKit's own heap. With an empty history the same page is 23 MB.
  The Home page holds 278 rows (the last 7 days of this history) in about 1,460 elements.
- **History.** Graphics memory grows with what History has shown: 41 MB of graphics with the
  first page, 90 MB in all once 1,128 rows were loaded by scrolling. Every loaded row stays in the
  page.
- **The focused search field.** History used to focus its search field on entry; that alone held
  about 21 MB of graphics memory (62 MB against 41 MB). This commit no longer focuses it on entry:
  typing anywhere in History goes to the field instead.
- **After close.** `hyperme` stays 7 to 9 MB higher and the GPU process about 6 MB higher. The
  closed window's native window stays allocated, off screen, until the next window event: it is
  freed when Home opens again, so at most one lingers and cycles do not grow. Running an empty task
  on the main thread after the close did not free it sooner.
- **Fixed cost outside Home.** At idle, `hyperme` holds an 8 MB decoded image (the shell
  measurement noted it too); the app icon is 512 px, about 1 MB decoded, so it is something else.

## Runs on the way

| Run | Change | Home page open | After close | Five cycles |
| --- | --- | --- | --- | --- |
| 1 | Slice 4 as pushed | 110.3 MB | 71.3 MB | 70.4 MB |
| 2 | Action bars drawn only on the selected row | 110.4 MB | 73.4 MB | 74.5 MB |
| 3 | The window's event handler no longer holds a clone of the window | 114.1 MB | 71.1 MB | 72.2 MB |
| 4 (final) | Search field no longer focused on entry | 113.4 MB | 74.3 MB | 72.4 MB |

Runs differ by 2 to 4 MB from run to run at the same step. In run 1 the History step loaded fewer
rows than later runs, so its History figure is left out. A layout where the document scrolls
instead of the page's main box lowered History's graphics to 30 MB but raised the Home page's to
31 MB, so it was not kept.
