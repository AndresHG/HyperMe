#!/bin/sh
# Builds HyperMe.app from the release binary and installs it in ~/Applications, so HyperMe has
# its own rows in System Settings > Privacy & Security instead of borrowing the terminal's.
#
#   scripts/bundle-macos.sh [--dest DIR] [--no-build]
#
# Signs with "HyperMe Local Signing" when scripts/create-signing-identity.sh has made it, so
# permission grants survive rebuilds. Without it, the app gets an ad-hoc signature and macOS
# asks for the permissions again after every rebuild. Local use only: not notarised.
set -eu

dest="$HOME/Applications"
build=1
while [ $# -gt 0 ]; do
  case "$1" in
    --dest) dest="$2"; shift 2 ;;
    --no-build) build=0; shift ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
[ "$build" = 1 ] && cargo build --release -p hyperme-app
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)

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
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <!-- A menu-bar app: no Dock icon (spec R-22). -->
  <key>LSUIElement</key><true/>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSMicrophoneUsageDescription</key><string>HyperMe listens while you hold the dictation key, and only then.</string>
</dict>
</plist>
PLIST

identity="HyperMe Local Signing"
if security find-identity -v -p codesigning | grep -q "\"$identity\""; then
  codesign --force --sign "$identity" --identifier org.hyperme.HyperMe "$bundle"
  signed="with \"$identity\" (grants survive rebuilds)"
else
  codesign --force --sign - --identifier org.hyperme.HyperMe "$bundle"
  signed="ad-hoc (run scripts/create-signing-identity.sh once so grants survive rebuilds)"
fi
codesign --verify "$bundle"

mkdir -p "$dest"
if pgrep -f "$app/Contents/MacOS/hyperme" >/dev/null; then
  echo "Quit HyperMe first (menu-bar icon > Quit HyperMe)." >&2
  exit 1
fi
rm -rf "$app"
mv "$bundle" "$app"
echo "Installed $app, signed $signed."
if ! security dump-trust-settings -d 2>/dev/null | grep -q "$identity"; then
  echo "Note: this build has a new signature hash, and macOS's system privacy service cannot check" >&2
  echo "the local identity until it is trusted for the whole Mac, so switch HyperMe off and on in" >&2
  echo "Accessibility after each rebuild. Run scripts/create-signing-identity.sh once to fix that." >&2
fi
echo "Start it with: open \"$app\""
