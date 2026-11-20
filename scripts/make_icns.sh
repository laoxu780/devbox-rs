#!/bin/bash
# Build an .icns from resources/AppIcon.png (macOS only).
set -euo pipefail
cd "$(dirname "$0")/.."
RES=resources
SET="$RES/AppIcon.iconset"
rm -rf "$SET" "$RES/AppIcon.icns"
mkdir -p "$SET"
for spec in "16 16" "16@2x 32" "32 32" "32@2x 64" "128 128" "128@2x 256" "256 256" "256@2x 512" "512 512" "512@2x 1024"; do
  set -- $spec
  sips -z "$2" "$2" "$RES/AppIcon.png" --out "$SET/icon_${1}x${1}.png" >/dev/null
done
iconutil -c icns "$SET" -o "$RES/AppIcon.icns"
rm -rf "$SET"
echo "wrote $RES/AppIcon.icns"
