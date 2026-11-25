# Contributing to HyperMe

Thank you for helping. HyperMe is small on purpose: fast, light, and easy to read. These rules
keep it that way. `CLAUDE.md` holds the full set for coding agents; this page is the part every
contributor needs.

## Before you start

- Look for an issue, or open one. Say what you want to change and why, before writing code.
- Read `docs/SPEC.md` for the architecture, budgets and safety rules, and the "Current phase"
  section of `CLAUDE.md`. Work that belongs to a later phase waits until its gate has passed.

## How a change is built: spec first, test first

1. **Size it.**
   - **S**: a bug with a repro, copy, config. Write the expected behaviour in the issue and go
     straight to a failing test.
   - **M**: one feature or behaviour inside one part. Write acceptance criteria in the issue as
     `AC-1`, `AC-2`, ...
   - **L**: a new part, a new provider or connector, a change to what leaves the machine, or work
     that needs more than one pull request. Write a spec in `specs/<issue>-<slug>/spec.md` and get
     it agreed before production code. See `specs/README.md`.
2. **Write the failing test.** Each test names the `AC-n` it proves, in a comment or doc comment.
   A bug fix starts with a test that reproduces the bug.
3. **Make it pass** with the simplest code that does.
4. **Measure** anything that touches a budget (latency, memory, CPU) and record the numbers in
   `docs/results/` with the date, machine and network.

## Checks

Run these in each crate you changed. CI runs the same, plus a secret scan and a licence and
advisory check (`deny.toml`).

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Tests must not call a paid API or need a key. Use the local fakes and test servers, as
`spikes/flowbench` does. On Linux, `cpal` needs the ALSA headers (`libasound2-dev`).

## Pull requests

- Branch from `master`; never commit to `master`. One topic per pull request.
- Fill in the template, including `## Lessons` ("None" is a valid answer).
- Small commits with clear messages. A commit that adds a crate says why it is needed.
- Anything you defer gets its own issue, linked from `## Not done`.

## What never goes in the repository

- Keys, tokens or passwords. Keys come from environment variables or the OS keychain. If one is
  committed by mistake, revoke it first, then tell the maintainer: deleting the file does not
  remove it from history.
- Audio, dictation transcripts, meeting notes or anything else a real person said. Test fixtures
  are synthetic. `docs/results/` holds numbers, not transcripts.
- Personal details about anyone, including the maintainer, and anything about an employer or
  client.
- GPL code. Dependencies must be MIT, Apache-2.0 or BSD (`deny.toml` lists the exceptions).
  Reading GPL projects for ideas is fine; copying from them is not. Code borrowed from a
  permissive project is recorded in `NOTICE.md` with its repository, commit and licence.

## Platforms

macOS comes first. Linux and Windows follow, so keep platform code behind a trait or a
`#[cfg(target_os = ...)]` module, and keep the core free of platform calls. CI runs on Linux and
macOS today; Windows joins when there is code for it.

## Licence

HyperMe is licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your
option. Unless you say otherwise, any contribution you submit for inclusion is dual-licensed the
same way, with no additional terms or conditions.
