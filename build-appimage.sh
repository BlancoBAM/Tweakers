#!/usr/bin/env bash
set -euo pipefail

APP=Tweakers
BIN=tweakers
ARCH="${ARCH:-x86_64}"
APPDIR="AppDir"

echo "→ Building release binary..."
cargo build --release

echo "→ Assembling AppDir..."
rm -rf "$APPDIR"
mkdir -p \
  "$APPDIR/usr/bin" \
  "$APPDIR/usr/share/applications" \
  "$APPDIR/usr/share/icons/hicolor/scalable/apps"

cp "target/release/$BIN"       "$APPDIR/usr/bin/"
cp "tweakers.desktop"          "$APPDIR/usr/share/applications/"

if [ -f icon.svg ]; then
  cp icon.svg "$APPDIR/usr/share/icons/hicolor/scalable/apps/tweakers.svg"
fi

# linuxdeploy check
BIN_DIR="$PWD/target/bin"
mkdir -p "$BIN_DIR"

if ! command -v linuxdeploy > /dev/null 2>&1 || ! command -v linuxdeploy-plugin-appimage > /dev/null 2>&1; then
  echo "Downloading linuxdeploy and appimage plugin..."
  
  curl -fsSL -o "$BIN_DIR/linuxdeploy" \
    "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage"
  chmod +x "$BIN_DIR/linuxdeploy"
  
  curl -fsSL -o "$BIN_DIR/linuxdeploy-plugin-appimage" \
    "https://github.com/linuxdeploy/linuxdeploy-plugin-appimage/releases/download/continuous/linuxdeploy-plugin-appimage-x86_64.AppImage"
  chmod +x "$BIN_DIR/linuxdeploy-plugin-appimage"
fi

export PATH="$BIN_DIR:$PATH"
export APPIMAGE_EXTRACT_AND_RUN=1

echo "→ Building AppImage..."
linuxdeploy \
  --appdir "$APPDIR" \
  --desktop-file tweakers.desktop \
  --icon-file icon.svg \
  --output appimage

echo "✓ AppImage created in project root."
