#!/usr/bin/env bash
# run-app.sh — launch osm-world wrapped in a macOS .app bundle.
#
# Why this exists: a raw binary launched from a terminal is not promoted to the
# frontmost/key macOS application, so keyboard and mouse events route to the
# terminal instead of the window. Under tmux (or any terminal multiplexer) it
# fails every time, because tmux's setsid() severs the "responsible process"
# chain macOS uses to decide app activation. Wrapping the binary in a .app
# bundle and launching it via `open` gives the process a real application
# identity, so focus and input work even from inside tmux.
#
# osm-world (wgpu/Metal) needs no MoltenVK/DYLD env vars, but it does load map
# and tile assets relative to the working directory, so the shim cd's into the
# repo root before exec'ing the binary.
#
# Usage: scripts/run-app.sh [--no-launch] [<app args...>]
#   --no-launch   build + assemble + validate the bundle but do not open it.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_NAME="osm-world"
BIN="$ROOT_DIR/target/release/$APP_NAME"
APP_DIR="$ROOT_DIR/.dev/$APP_NAME.app"
MACOS_DIR="$APP_DIR/Contents/MacOS"
SHIM="$MACOS_DIR/$APP_NAME"
INFO_PLIST="$APP_DIR/Contents/Info.plist"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT_DIR/Cargo.toml" | head -1)"

NO_LAUNCH=0
if [[ "${1:-}" == "--no-launch" ]]; then NO_LAUNCH=1; shift; fi

cd "$ROOT_DIR"
cargo build --release

rm -rf "$APP_DIR"
mkdir -p "$MACOS_DIR"

cat > "$INFO_PLIST" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>$APP_NAME</string>
  <key>CFBundleDisplayName</key><string>$APP_NAME</string>
  <key>CFBundleIdentifier</key><string>com.paulrobello.$APP_NAME.dev</string>
  <key>CFBundleExecutable</key><string>$APP_NAME</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>LSUIElement</key><false/>
  <key>LSMinimumSystemVersion</key><string>10.13</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

cat > "$SHIM" <<SHIM
#!/bin/bash
cd "$ROOT_DIR"
exec "$BIN" "\$@"
SHIM
chmod +x "$SHIM"

if command -v plutil >/dev/null 2>&1; then plutil -lint "$INFO_PLIST" >/dev/null; fi
[[ -x "$SHIM" && -x "$BIN" ]]

if [[ "$NO_LAUNCH" == "1" ]]; then
  echo "run-app: bundle assembled at $APP_DIR (not launched)"
  exit 0
fi

echo "run-app: launching $APP_DIR"
open -n "$APP_DIR" --args "$@"
