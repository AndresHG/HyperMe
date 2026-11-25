# Phase 1 slices

Each slice is one pull request, starts from failing tests that cite its `AC-n`, and passes fmt,
clippy and tests before it merges. Open one issue per slice, labelled `phase-1` and its area,
when the spec is approved and the Phase 0 gate has passed. Slices marked **good first issue**
are small, well bounded, and need no Mac-specific knowledge.

| # | Slice | Proves | Area | Depends on |
| --- | --- | --- | --- | --- |
| 1 | Workspace and empty Tauri 2 menu-bar app with the pill panel, no Dock icon; CI matrix entries; measure idle memory (design.md, "Memory budget check") | R-22, R-23 in part; budget check | `area: app` | none |
| 2 | `hyperme-core`: config file, parsing and validation, with clear errors. **Good first issue** | R-26, AC-18 (config half) | `area: core` | 1 |
| 3 | `hyperme-core`: the dictation state machine with all traits faked | AC-1, AC-4, AC-5, AC-6, AC-9, AC-11 | `area: core` | 2 |
| 4 | `hyperme-providers`: Speechmatics streaming, ported from flowbench, with a local test server | AC-3, AC-6, AC-7, AC-16 (speech) | `area: speech` | 3 |
| 5 | `hyperme-providers`: OpenAI-compatible cleanup with warm-up, deadline and fallback, with a local test server | AC-8, AC-16 (cleanup) | `area: cleanup` | 3 |
| 6 | `hyperme-platform` macOS: microphone capture and resampling | AC-2 | `area: audio` | 3 |
| 7 | `hyperme-platform` macOS: hold-to-talk hotkey and Esc | R-1, R-6, R-8 | `area: input` | 3 |
| 8 | `hyperme-platform` macOS: paste at the cursor, clipboard restore, secure-input check, frontmost app | AC-12, AC-13, AC-14 | `area: input` | 3 |
| 9 | Keychain `SecretStore` and the settings window | AC-18, R-24, R-25 | `area: app` | 2 |
| 10 | Pill states and the menu-bar menu, including "Paste last" and "Copy last" | AC-15, AC-17 | `area: ui` | 1, 3 |
| 11 | First-launch walkthrough for permissions and keys | AC-21 | `area: app` | 6 to 10 |
| 12 | Timing log and the live measurement: AC-2, AC-10, AC-19 written to `docs/results/` | AC-2, AC-10, AC-19, AC-20 | `area: core` | all |

## Good first issues outside the slices

These can be opened now, before the gate, because they touch no phase work:

- A `--json` flag for flowbench's report, once the tuning work there has merged (ask in
  the issue first, since that code is moving).
- A pill state mock-up as a static HTML page in `docs/design/`, from the states in design.md.
  **Good first issue**, no Rust needed.
- A script or test that checks every relative link in `docs/` and `specs/` resolves. **Good first
  issue**.
