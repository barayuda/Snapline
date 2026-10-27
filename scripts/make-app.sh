#!/bin/zsh
# Builds build/Snapline.app (arm64, release) and signs it ad hoc.
# Usage: scripts/make-app.sh [--install]   (--install copies it to ~/Applications)
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="1.0.0"
APP="build/Snapline.app"

swift build -c release --arch arm64
BIN="$(swift build -c release --arch arm64 --show-bin-path)/Snapline"

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/Snapline"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key><string>dev.barayuda.snapline</string>
  <key>CFBundleName</key><string>Snapline</string>
  <key>CFBundleDisplayName</key><string>Snapline</string>
  <key>CFBundleExecutable</key><string>Snapline</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key><string>15.0</string>
  <key>LSUIElement</key><true/>
  <key>LSApplicationCategoryType</key><string>public.app-category.productivity</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSHumanReadableCopyright</key><string>Barayuda</string>
</dict>
</plist>
PLIST

# Ad-hoc signature. macOS ties the Screen Recording permission to it, so after
# every rebuild you may have to switch Snapline off and on again in System Settings.
codesign --force --sign - --identifier dev.barayuda.snapline "$APP"
codesign --verify --strict "$APP"
lipo -info "$APP/Contents/MacOS/Snapline"

if [[ "${1:-}" == "--install" ]]; then
  mkdir -p ~/Applications
  rm -rf ~/Applications/Snapline.app
  cp -R "$APP" ~/Applications/
  echo "Installed to ~/Applications/Snapline.app"
fi
