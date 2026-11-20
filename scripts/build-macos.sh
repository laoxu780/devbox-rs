#!/bin/bash
# Build a self-contained DevBox.app bundle for macOS.
set -euo pipefail
cd "$(dirname "$0")/.."

if [ ! -f resources/AppIcon.icns ]; then
  ./scripts/make_icns.sh
fi

cargo build --release

APP=dist/DevBox.app
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

cp target/release/devbox "$APP/Contents/MacOS/devbox"
cp resources/AppIcon.icns "$APP/Contents/Resources/DevBox.icns"

cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>zh_CN</string>
  <key>CFBundleExecutable</key><string>devbox</string>
  <key>CFBundleIconFile</key><string>DevBox.icns</string>
  <key>CFBundleIdentifier</key><string>com.devbox.toolkit</string>
  <key>CFBundleDisplayName</key><string>DevBox</string>
  <key>CFBundleName</key><string>DevBox</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleVersion</key><string>0.1.0</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

# Ad-hoc signing so Finder trusts the bundle structure. Remove the quarantine
# attribute if the zip was downloaded from the internet.
if command -v codesign >/dev/null 2>&1; then
  codesign --force --deep --sign - "$APP" >/dev/null 2>&1 || true
fi

echo "built $APP"
