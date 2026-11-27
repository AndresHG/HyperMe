# Releasing HyperMe

How a HyperMe release is built, signed, notarised and published, and how the app updates itself.
GitHub Actions is blocked by billing, so every release is made on the owner's Mac with
`scripts/release-macos.sh`. On 27 November 2026 the owner chose not to enrol in the Apple
Developer Program for now: releases are published **unsigned** (signed with the local identity,
not notarised), the way VFS ships its builds, and Developer ID waits for a later decision. The
Apple steps below are kept for that day.

## What a release contains

`scripts/release-macos.sh` writes these to `dist/v<version>/` (ignored by git):

| File | What it is |
| --- | --- |
| `HyperMe_<version>_aarch64.dmg` | The download: HyperMe.app and a link to Applications. Signed with the local identity for now; with Developer ID, also notarised and stapled |
| `HyperMe_<version>_aarch64.app.tar.gz` | The same app, for the in-app updater |
| `HyperMe_<version>_aarch64.app.tar.gz.sig` | Its updater signature (minisign, made with the HyperMe updater key) |
| `latest.json` | The update manifest: version, notes, date, and the archive's URL and signature |
| `SHA256SUMS` | Checksums of the DMG and the archive |

Only Apple silicon (`aarch64`) for now; an Intel or universal build is a follow-up.

## How updates work

- The menu-bar menu has **Check for updates…**. HyperMe checks only when the user clicks it, and
  installs only after a second click on **Install HyperMe x.y.z and restart**. There is no check
  in the background, so nothing leaves the Mac on its own (CLAUDE.md, no telemetry).
- The check downloads
  `https://github.com/AndresHG/hyperme/releases/latest/download/latest.json`, which GitHub serves
  from the newest published release. Drafts and pre-releases are not seen.
- The download is verified against the public key in `app/src-tauri/tauri.conf.json` before
  anything is replaced. `requireSignedVersion` is on: the signature must name the version the
  manifest announces, so an older signed build cannot be passed off as a newer one.
- Tauri's updater plugin replaces HyperMe.app in place and restarts it. Every published version
  is signed with the same identity ("HyperMe Local Signing" now, Developer ID later), so the
  Accessibility, Input Monitoring and Microphone grants should survive updates; not yet checked on
  a second Mac. The update is downloaded by the app, not a browser, so Gatekeeper does not block
  it a second time. Moving from the local identity to Developer ID later changes the signature
  once: users grant the permissions again after that one update.
- The repository is private until launch, so the endpoint answers 404 until then and the menu
  shows "Update failed". That is expected.

## One-time setup

### Updater key (done 26 November 2026)

The key that signs updates. It is free and has nothing to do with Apple.

- Private key: `~/.tauri/hyperme-updater.key` on the owner's Mac, encrypted with a password.
- Password: login keychain, item `hyperme-updater-key`, account `hyperme`.
- Public key: `plugins.updater.pubkey` in `app/src-tauri/tauri.conf.json`.

**Back up the private key file and its password** (a password manager is fine). If the key is
lost, installed copies can never update again: users would have to download a new DMG by hand.
Never commit it, paste it into an issue, or put it in a cloud copy of the repository.

Made with:

```sh
cargo install tauri-cli --version 2.12.6 --locked
pw=$(openssl rand -hex 24)
security add-generic-password -U -s hyperme-updater-key -a hyperme -w "$pw"
cargo tauri signer generate --ci -p "$pw" -w ~/.tauri/hyperme-updater.key
unset pw
```

### Local signing identity (done)

`scripts/create-signing-identity.sh` made "HyperMe Local Signing", a self-signed certificate in
the owner's login keychain. Unsigned releases are signed with it, and the script refuses to
publish without it. **Back it up with the updater key** (Keychain Access > export as .p12): a new
identity would make every installed copy lose its permission grants on the next update.

### Apple Developer ID (deferred by the owner, 27 November 2026)

1. Enrol in the Apple Developer Program as an individual (developer.apple.com/programs).
2. In Xcode, Settings > Accounts > Manage Certificates, add a **Developer ID Application**
   certificate. It lands in the login keychain; `security find-identity -v -p codesigning` then
   lists `Developer ID Application: <name> (<team id>)`.
3. Make an app-specific password at account.apple.com (Sign-In and Security), and store the
   notarisation credentials in the keychain once:

   ```sh
   xcrun notarytool store-credentials hyperme-notary \
     --apple-id <apple id> --team-id <team id> --password <app-specific password>
   ```

   The release script reads the profile `hyperme-notary`; nothing is written to the repository.

## Making a release

1. Bump `version` in the root `Cargo.toml` (the app's version comes from there only) and move the
   `[Unreleased]` entries in `CHANGELOG.md` under the new version. Merge that pull request.
2. On an up-to-date, clean `master`:

   ```sh
   scripts/release-macos.sh --unsigned --notes notes.md --publish
   ```

   It runs fmt, clippy and the tests; builds HyperMe.app and signs it with the local identity;
   makes the DMG; signs the update; checks the app with `codesign`; and uploads a **draft**
   release `v<version>` with `gh`. The draft's notes add how to open an app that is not
   notarised (the same steps as the README).

   With Developer ID set up, drop `--unsigned`: the script then signs with the hardened runtime,
   notarises and staples the app and the DMG, and checks them with `spctl` and `stapler` too.
3. Open the draft on GitHub, read it, and publish it. Publishing is what makes the update visible
   to installed copies.
4. Tag check: `gh release view v<version>` shows the five files.

### Trial run

```sh
scripts/release-macos.sh --unsigned
```

Builds everything into `dist/` and uploads nothing. Without the local identity it signs ad hoc,
which is fine for a trial but never published.

### What users see with an unsigned build

macOS blocks the first open of an app that is not notarised. The user opens it once from
System Settings > Privacy & Security > **Open Anyway** (on recent macOS, right-click > Open no
longer offers this), or runs `xattr -dr com.apple.quarantine /Applications/HyperMe.app`. Later
updates from the menu install without this step.

## Settings

All optional; none is a secret.

| Variable | Default |
| --- | --- |
| `HYPERME_SIGN_IDENTITY` | The only `Developer ID Application` identity in the keychain |
| `HYPERME_NOTARY_PROFILE` | `hyperme-notary` |
| `HYPERME_UPDATER_KEY` | `~/.tauri/hyperme-updater.key` |
