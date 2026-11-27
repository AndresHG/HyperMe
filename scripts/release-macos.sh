#!/bin/sh
# Builds a HyperMe release on this Mac: the checks, a signed and notarised HyperMe.app, a DMG, and
# the signed update the in-app updater installs. GitHub Actions is blocked by billing, so releases
# are made here (docs/RELEASING.md has the one-time setup and the whole procedure).
#
#   scripts/release-macos.sh [--unsigned] [--skip-checks] [--notes FILE] [--publish]
#
#   --unsigned     No Apple Developer ID (the owner's choice for now, 27 November 2026): signs
#                  with "HyperMe Local Signing" (or ad hoc for a trial), skips notarisation, and
#                  still makes the DMG and the signed update. Gatekeeper warns on first open; the
#                  README says how to open it. With --publish it needs the local identity, so
#                  every version keeps one signature and permission grants survive updates.
#   --skip-checks  Skip cargo fmt, clippy and test (they must have passed on this commit).
#   --notes FILE   Release notes for latest.json and the GitHub release (default: a link to the
#                  changelog).
#   --publish      Upload to a draft GitHub release v<version> with gh. The owner reviews the draft
#                  and publishes it; only a published release reaches the updater.
#
# Settings, from the environment (none of them is a secret):
#   HYPERME_SIGN_IDENTITY   Developer ID Application identity (default: the only one in the
#                           keychain)
#   HYPERME_NOTARY_PROFILE  notarytool keychain profile (default: hyperme-notary)
#   HYPERME_UPDATER_KEY     the updater's private key file (default: ~/.tauri/hyperme-updater.key)
# The updater key's password is read from the login keychain item "hyperme-updater-key".
set -eu

unsigned=0
checks=1
notes_file=""
publish=0
while [ $# -gt 0 ]; do
  case "$1" in
    --unsigned) unsigned=1; shift ;;
    --skip-checks) checks=0; shift ;;
    --notes) notes_file="$2"; shift 2 ;;
    --publish) publish=1; shift ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
say() { printf '\n==> %s\n' "$*"; }
need() { command -v "$1" >/dev/null || { echo "missing: $1 ($2)" >&2; exit 1; }; }

need cargo "https://rustup.rs"
need codesign "Xcode command line tools"
need hdiutil "macOS"
cargo tauri --version >/dev/null 2>&1 \
  || { echo "missing: cargo tauri (cargo install tauri-cli --version 2.12.6 --locked)" >&2; exit 1; }

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
tag="v$version"
case "$(uname -m)" in
  arm64) arch=aarch64 ;;
  x86_64) arch=x86_64 ;;
  *) echo "unsupported CPU: $(uname -m)" >&2; exit 1 ;;
esac
out="dist/$tag"

# A published release must be traceable to one clean commit.
if { [ "$unsigned" = 0 ] || [ "$publish" = 1 ]; } && [ -n "$(git status --porcelain --untracked-files=no)" ]; then
  echo "Commit or stash your changes first: a release is built from a clean tree." >&2
  exit 1
fi

# Signing settings, checked before the long build.
key_file="${HYPERME_UPDATER_KEY:-$HOME/.tauri/hyperme-updater.key}"
[ -f "$key_file" ] || { echo "No updater key at $key_file (docs/RELEASING.md, one-time setup)." >&2; exit 1; }
key_password=$(security find-generic-password -s hyperme-updater-key -a hyperme -w 2>/dev/null) \
  || { echo "No keychain item hyperme-updater-key (docs/RELEASING.md, one-time setup)." >&2; exit 1; }
identity=""
profile="${HYPERME_NOTARY_PROFILE:-hyperme-notary}"
if [ "$unsigned" = 0 ]; then
  identity="${HYPERME_SIGN_IDENTITY:-}"
  if [ -z "$identity" ]; then
    count=$(security find-identity -v -p codesigning | grep -c '"Developer ID Application:' || true)
    if [ "$count" != 1 ]; then
      echo "Found $count Developer ID Application identities; set HYPERME_SIGN_IDENTITY." >&2
      echo "With no Apple Developer account yet, try --unsigned." >&2
      exit 1
    fi
    identity=$(security find-identity -v -p codesigning | sed -n 's/.*"\(Developer ID Application:[^"]*\)".*/\1/p')
  fi
  need xcrun "Xcode"
  xcrun notarytool history --keychain-profile "$profile" >/dev/null 2>&1 \
    || { echo "notarytool profile \"$profile\" is not set up (docs/RELEASING.md)." >&2; exit 1; }
elif [ "$publish" = 1 ]; then
  # An ad-hoc signature changes with every build, and macOS would drop the user's grants on each
  # update. The local identity keeps one signature across versions.
  security find-identity -v -p codesigning | grep -q '"HyperMe Local Signing"' \
    || { echo "Publishing an unsigned build needs \"HyperMe Local Signing\" (scripts/create-signing-identity.sh)." >&2; exit 1; }
fi

if [ "$checks" = 1 ]; then
  say "Checks: fmt, clippy, tests"
  cargo fmt --check
  cargo clippy --all-targets -- -D warnings
  cargo test
fi

say "HyperMe.app $version ($arch)"
rm -rf "$out"
mkdir -p "$out"
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
if [ -n "$identity" ]; then
  scripts/bundle-macos.sh --dest "$stage" --identity "$identity"
else
  scripts/bundle-macos.sh --dest "$stage"
fi
app="$stage/HyperMe.app"

if [ -n "$identity" ]; then
  say "Notarising the app (a few minutes)"
  ditto -c -k --keepParent "$app" "$stage/HyperMe.zip"
  xcrun notarytool submit "$stage/HyperMe.zip" --keychain-profile "$profile" --wait
  xcrun stapler staple "$app"
fi

say "DMG"
dmg="$out/HyperMe_${version}_${arch}.dmg"
mkdir "$stage/dmg"
cp -R "$app" "$stage/dmg/"
ln -s /Applications "$stage/dmg/Applications"
hdiutil create -volname "HyperMe $version" -srcfolder "$stage/dmg" -fs HFS+ -format UDZO -ov "$dmg" >/dev/null
if [ -n "$identity" ]; then
  codesign --force --sign "$identity" --timestamp "$dmg"
  xcrun notarytool submit "$dmg" --keychain-profile "$profile" --wait
  xcrun stapler staple "$dmg"
fi

say "Signed update"
archive="HyperMe_${version}_${arch}.app.tar.gz"
# The updater expects HyperMe.app at the top of the archive.
COPYFILE_DISABLE=1 tar -czf "$out/$archive" -C "$stage" HyperMe.app
# --app-version binds the signature to this version (requireSignedVersion in tauri.conf.json).
TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$key_password" \
  cargo tauri signer sign -f "$key_file" --app-version "$version" "$out/$archive" >/dev/null
unset key_password
signature=$(cat "$out/$archive.sig")

if [ -n "$notes_file" ]; then
  notes=$(cat "$notes_file")
else
  notes="See https://github.com/AndresHG/hyperme/blob/master/CHANGELOG.md"
fi
# latest.json: Tauri's static update format. The URL points at the asset of the same release.
python3 - "$out/latest.json" "$version" "$notes" "$arch" \
  "https://github.com/AndresHG/hyperme/releases/download/$tag/$archive" "$signature" <<'PY'
import datetime, json, sys
path, version, notes, arch, url, signature = sys.argv[1:]
manifest = {
    "version": version,
    "notes": notes,
    "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "platforms": {f"darwin-{arch}": {"signature": signature, "url": url}},
}
with open(path, "w") as f:
    json.dump(manifest, f, indent=2)
    f.write("\n")
PY

say "Verifying"
codesign --verify --deep --strict "$app"
if [ -n "$identity" ]; then
  spctl --assess --type execute "$app"
  spctl --assess --type open --context context:primary-signature "$dmg"
  xcrun stapler validate "$dmg"
fi
(cd "$out" && shasum -a 256 ./*.dmg ./*.tar.gz > SHA256SUMS)
ls -l "$out"

if [ "$publish" = 1 ]; then
  need gh "https://cli.github.com"
  say "Draft GitHub release $tag"
  release_notes="$notes"
  if [ "$unsigned" = 1 ]; then
    release_notes="$notes

This build is not notarised by Apple, so macOS blocks it the first time. Open it once from System Settings > Privacy & Security > Open Anyway, or run \`xattr -dr com.apple.quarantine /Applications/HyperMe.app\`. Updates from the app menu install without this step."
  fi
  gh release create "$tag" --draft --title "HyperMe $version" --notes "$release_notes" \
    --target "$(git rev-parse HEAD)" \
    "$dmg" "$out/$archive" "$out/$archive.sig" "$out/latest.json" "$out/SHA256SUMS"
  echo "Review the draft on GitHub, then publish it. The updater sees only published releases."
elif [ "$unsigned" = 1 ]; then
  echo "Unsigned build in $out (not notarised). Run again with --unsigned --publish --skip-checks to upload a draft."
else
  echo "Release $version is in $out. Run again with --publish --skip-checks to upload a draft."
fi
