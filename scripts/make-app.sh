#!/bin/zsh
# Builds build/Snapline.app (arm64, release) and signs it ad hoc.
# Usage: scripts/make-app.sh [--install]   (--install replaces /Applications/Snapline.app and starts it)
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="1.0.0"
APP="build/Snapline.app"

swift build -c release --arch arm64
BIN="$(swift build -c release --arch arm64 --show-bin-path)/Snapline"

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/Snapline"
# Layered icon (Assets.car for macOS 26+, AppIcon.icns for older). Regenerate art with: swift scripts/brand/make-brand.swift
xcrun actool "$PWD/Assets/AppIcon.icon" --compile "$PWD/$APP/Contents/Resources" --platform macosx --target-device mac \
  --minimum-deployment-target 15.0 --app-icon AppIcon --output-partial-info-plist "$PWD/build/icon-partial.plist" >/dev/null  # actool needs absolute paths

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key><string>dev.barayuda.snapline</string>
  <key>CFBundleName</key><string>Snapline</string>
  <key>CFBundleDisplayName</key><string>Snapline</string>
  <key>CFBundleExecutable</key><string>Snapline</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundleIconName</key><string>AppIcon</string>
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

# Tell Launch Services about the new bundle, so Finder, Dock and System Settings show the current icon.
LSREGISTER=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister
"$LSREGISTER" -f "$APP"

if [[ "${1:-}" == "--install" ]]; then
  # Quit the running copy, replace /Applications/Snapline.app, register it and start it again.
  pkill -x Snapline 2>/dev/null && sleep 1 || true
  rm -rf /Applications/Snapline.app
  cp -R "$APP" /Applications/
  "$LSREGISTER" -f /Applications/Snapline.app
  open /Applications/Snapline.app
  echo "Installed and started /Applications/Snapline.app"
fi
