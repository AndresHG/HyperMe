#!/bin/sh
# Builds HyperMe.app from the release binary and installs it in ~/Applications, so HyperMe has
# its own rows in System Settings > Privacy & Security instead of borrowing the terminal's.
#
#   scripts/bundle-macos.sh [--dest DIR] [--no-build] [--identity NAME]
#
# Signs with "HyperMe Local Signing" when scripts/create-signing-identity.sh has made it, so
# permission grants survive rebuilds. Without it, the app gets an ad-hoc signature and macOS
# asks for the permissions again after every rebuild. Local use only: not notarised.
#
# --identity signs with that identity instead, with the hardened runtime, a secure timestamp
# and app/src-tauri/entitlements.plist, as notarisation needs. scripts/release-macos.sh passes
# the Developer ID Application identity here (docs/RELEASING.md).
set -eu

dest="$HOME/Applications"
build=1
release_identity=""
while [ $# -gt 0 ]; do
  case "$1" in
    --dest) dest="$2"; shift 2 ;;
    --no-build) build=0; shift ;;
    --identity) release_identity="$2"; shift 2 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
[ "$build" = 1 ] && cargo build --release -p hyperme-app
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
# The commit the app is built from, with "-dirty" when the tree has changes, so the installed app
# can always be traced: defaults read ~/Applications/HyperMe.app/Contents/Info HyperMeGitCommit
commit=$(git rev-parse --short HEAD 2>/dev/null || echo unknown)
[ -n "$(git status --porcelain --untracked-files=no 2>/dev/null)" ] && commit="$commit-dirty"

app="$dest/HyperMe.app"
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
bundle="$stage/HyperMe.app"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
cp target/release/hyperme "$bundle/Contents/MacOS/hyperme"

# The icon, from the app's PNG, with macOS's own tools.
set_dir="$stage/HyperMe.iconset"
mkdir -p "$set_dir"
for size in 16 32 128 256 512; do
  sips -z $size $size app/src-tauri/icons/icon.png --out "$set_dir/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  [ $double -le 512 ] && sips -z $double $double app/src-tauri/icons/icon.png --out "$set_dir/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$set_dir" -o "$bundle/Contents/Resources/HyperMe.icns"

cat > "$bundle/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>HyperMe</string>
  <key>CFBundleDisplayName</key><string>HyperMe</string>
  <key>CFBundleIdentifier</key><string>org.hyperme.HyperMe</string>
  <key>CFBundleExecutable</key><string>hyperme</string>
  <key>CFBundleIconFile</key><string>HyperMe</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>HyperMeGitCommit</key><string>$commit</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <!-- A menu-bar app: no Dock icon (spec R-22). -->
  <key>LSUIElement</key><true/>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSMicrophoneUsageDescription</key><string>HyperMe listens while you hold the dictation key, and only then.</string>
</dict>
</plist>
PLIST

identity="HyperMe Local Signing"
if [ -n "$release_identity" ]; then
  codesign --force --sign "$release_identity" --identifier org.hyperme.HyperMe \
    --options runtime --timestamp --entitlements app/src-tauri/entitlements.plist "$bundle"
  signed="with \"$release_identity\" (hardened runtime)"
elif security find-identity -v -p codesigning | grep -q "\"$identity\""; then
  codesign --force --sign "$identity" --identifier org.hyperme.HyperMe "$bundle"
  signed="with \"$identity\" (grants survive rebuilds)"
else
  codesign --force --sign - --identifier org.hyperme.HyperMe "$bundle"
  signed="ad-hoc (run scripts/create-signing-identity.sh once so grants survive rebuilds)"
fi
codesign --verify --strict "$bundle"

mkdir -p "$dest"
if pgrep -f "$app/Contents/MacOS/hyperme" >/dev/null; then
  echo "Quit HyperMe first (menu-bar icon > Quit HyperMe)." >&2
  exit 1
fi
rm -rf "$app"
mv "$bundle" "$app"
echo "Installed $app (commit $commit), signed $signed."
echo "Start it with: open \"$app\""
