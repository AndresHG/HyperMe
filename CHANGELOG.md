# Changelog

All notable changes to HyperMe are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/) once there is a release.

## [Unreleased]

### Added

- `spikes/flowbench`, a command-line speed test for dictation: upload against streaming,
  several speech and cleanup providers, stage timings, a quality score against gold texts.
- Phase 0 results in `docs/results/`: Speechmatics (EU) with Groq cleanup reaches a median of
  429 ms and a 90th percentile of 595 ms from key release to cleaned text on 50 samples.
- Project documents: spec, roadmap, research, pitch, contributing rules, security policy, code
  of conduct, issue and pull-request templates, CI.
- Licence: MIT OR Apache-2.0.
