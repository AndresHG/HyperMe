# Going public: checklist

What the repository needs before it is made public, and before the first release. Kept as a
checklist so a later session can tick it off. State on 25 November 2026.

## In the repository

| Item | State |
| --- | --- |
| Licence, MIT OR Apache-2.0 | Done (PR 22) |
| README with pitch, status and positioning | Done; copilot-first headline chosen by the owner ([PITCH.md](PITCH.md)) |
| Pitch, elevator pitch, claims ledger | Done: [PITCH.md](PITCH.md) |
| Contributing, spec-first and test-first rules | Done (PR 22), labels added |
| Code of conduct | Done: Contributor Covenant 2.1; reports through GitHub private reporting (owner's choice) |
| Security policy | Done (PR 22) |
| Issue templates and pull-request template | Done |
| CODEOWNERS | Done |
| Changelog | Done |
| Notice file for borrowed code | Done, empty |
| CI: fmt, clippy, tests, secret scan, licence policy | Done (PR 22). GitHub Actions blocked by billing on 25 November 2026; checks run locally until it is fixed |
| Roadmap and phase issues | Done ([ROADMAP.md](ROADMAP.md), #24 to #29); adopted by the owner on 25 November 2026 |
| Phase 1 spec | Draft: [specs/25-dictation-core](../specs/25-dictation-core/spec.md) |
| No personal, employer or transcript data in the full history | Read on 25 November 2026: no secrets, keys, audio, transcripts or client figures; the employer is never named. Master is clean; older commits still describe the owner's employer in general terms, which the owner accepts. Older commits also contain internal names that must never be published, and GitHub keeps old pull-request refs. Decision: publish a fresh repository with a clean first commit made from master; this repository stays private and its history is not rewritten (owner, 26 November 2026, replacing the earlier same-day decision to go public as it is) |

## Repository settings (owner, on GitHub)

These are settings, not files, and only matter once the repository is public:

- [ ] Enable private vulnerability reporting (Settings, Security). `SECURITY.md` and the code of
      conduct both point to it; GitHub offers it only on public repositories.
- [ ] Description (the one in [PITCH.md](PITCH.md); this session could not change repository settings) and topics, for example `dictation`, `voice`,
      `speech-to-text`, `tauri`, `rust`, `macos`, `wispr-flow-alternative`.
- [ ] Social preview image.
- [ ] Branch protection on `master`: pull request required, CI green required.
- [ ] Discussions: on or off. Off keeps one place for questions (issues) while the project is
      small.
- [ ] Turn macOS CI on for pull requests (it switches on by itself when the repository is public).

## Before the first release (Phase 2)

- [ ] Signed and notarised build, and a download on the Releases page.
- [ ] Every README claim earned in the ledger in [PITCH.md](PITCH.md), with its evidence linked.
- [ ] Install and first-run instructions tested on a fresh macOS account.
- [ ] Launch post draft, single-purpose ("open-source dictation for your Mac"), with the method
      and numbers.
