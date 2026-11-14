#!/usr/bin/env bash
# Builds the .deb and the AppImage for this machine's architecture into target/dist, with checksums.
# Usage: linux/package.sh <version>      (run in windows-linux/, after `cargo build --release -p snapline`)
set -euo pipefail
cd "$(dirname "$0")/.."
version=$1
target=${CARGO_TARGET_DIR:-target}
arch=$(uname -m) # x86_64 or aarch64
deb_arch=$([ "$arch" = x86_64 ] && echo amd64 || echo arm64)
out=$target/dist
mkdir -p "$out"

cargo deb -p snapline --no-build --deb-version "$version" -o "$out/snapline_${version}_${deb_arch}.deb"

# AppImage: linuxdeploy copies snapline and the libraries it links (Tesseract, Leptonica) into an AppDir; the English
# data goes where Snapline looks for it ($APPDIR/usr/share/tesseract-ocr/5/tessdata).
appdir=$target/AppDir
rm -rf "$appdir"
install -Dm644 /usr/share/tesseract-ocr/5/tessdata/eng.traineddata "$appdir/usr/share/tesseract-ocr/5/tessdata/eng.traineddata"
tool=$target/linuxdeploy-$arch.AppImage
if [ ! -x "$tool" ]; then
  curl -fsSL -o "$tool" "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-$arch.AppImage"
  chmod +x "$tool"
fi
sha256sum "$tool" # the tool comes from a moving "continuous" release: keep its checksum in the log
APPIMAGE_EXTRACT_AND_RUN=1 LDAI_OUTPUT="$out/Snapline-$version-$arch.AppImage" "$tool" --appdir "$appdir" \
  --executable "$target/release/snapline" --desktop-file linux/dev.barayuda.Snapline.desktop \
  --icon-file linux/dev.barayuda.Snapline.png --output appimage

(cd "$out" && sha256sum -- *.deb *.AppImage >"SHA256SUMS-linux-$arch.txt")
