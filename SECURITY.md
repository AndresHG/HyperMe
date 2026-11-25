# Security

HyperMe handles microphone audio, text from other apps, and API keys for speech and language
model providers. Please report anything that could expose them.

## Reporting a vulnerability

Use GitHub's private vulnerability reporting: the **Security** tab of this repository, then
**Report a vulnerability**. Please do not open a public issue for a security problem.

You will get an answer within a week. This is a spare-time project, so there is no bounty.

## What HyperMe promises

From `docs/SPEC.md` and `CLAUDE.md`:

- Keys live in the OS keychain or environment variables, never in files, logs or commits.
- No telemetry. No audio is kept on disk.
- Audio, transcript, dictionary, app name and the text before the cursor may leave the machine,
  only to the providers the user configured with their own keys. Window text and screenshots
  only behind a visible switch that is off by default. Never password fields.
- Anything that creates, sends or posts outside the app shows a preview and waits for
  confirmation.

A way to break any of these is a vulnerability.
