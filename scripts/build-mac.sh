#!/usr/bin/env bash
# Build a release .app for macOS, ad-hoc signed so Apple Silicon will run it.
# Must run on macOS (Tauri links against Apple SDK + WebKit; no cross-compile).
#
#   ./scripts/build-mac.sh            # build + sign + zip
#   ./scripts/build-mac.sh --test     # run the Rust test suite first
#
# Output: dist/Trio-<version>-macos-<arch>.zip  (+ the .app next to it)
set -euo pipefail

cd "$(dirname "$0")/.."
ROOT="$PWD"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "error: this script must run on macOS (uname -s = $(uname -s))" >&2
  exit 1
fi

ARCH="$(uname -m)"
case "$ARCH" in
  arm64) BUNDLE_ARCH="aarch64" ;;
  x86_64) BUNDLE_ARCH="x86_64" ;;
  *) echo "error: unsupported arch $ARCH" >&2; exit 1 ;;
esac

echo "==> toolchain"
command -v cargo >/dev/null || { echo "error: cargo not found (install rustup)" >&2; exit 1; }
cargo --version
rustc --version

if ! command -v cargo-tauri >/dev/null; then
  echo "==> installing tauri-cli (one time, a few minutes)"
  cargo install tauri-cli --version "^2.0.0" --locked
fi
TAURI_VERSION="$(cargo tauri --version)"
echo "    $TAURI_VERSION"

if [[ "${1:-}" == "--test" ]]; then
  echo "==> cargo test"
  cargo test --manifest-path "$ROOT/src-tauri/Cargo.toml"
fi

VERSION="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["version"])' "$ROOT/src-tauri/tauri.conf.json")"
echo "==> building Trio $VERSION for $ARCH"

cd "$ROOT"
cargo tauri build --bundles app

APP="$ROOT/src-tauri/target/release/bundle/macos/Trio.app"
[[ -d "$APP" ]] || { echo "error: expected bundle not found: $APP" >&2; exit 1; }

echo "==> ad-hoc signing (no Developer ID needed for local testing)"
codesign --force --deep --sign - "$APP"
codesign --verify --deep --strict --verbose=2 "$APP" 2>&1 | sed 's/^/    /'

mkdir -p "$ROOT/dist"
ZIP="$ROOT/dist/Trio-$VERSION-macos-$BUNDLE_ARCH.zip"
rm -f "$ZIP"
# ditto preserves the bundle's symlinks, permissions and extended attributes.
ditto -c -k --keepParent "$APP" "$ZIP"

echo
echo "==> done"
echo "    app : $APP"
echo "    zip : $ZIP"
echo "    sha256: $(shasum -a 256 "$ZIP" | awk '{print $1}')"
echo
echo "Install: unzip into /Applications, then"
echo "    xattr -dr com.apple.quarantine /Applications/Trio.app"
echo "    open /Applications/Trio.app"
echo "First run: System Settings > Privacy & Security > Accessibility > enable Trio,"
echo "and turn OFF Trackpad > 'Look up & data detectors' > 'Tap with three fingers'."
echo "Debug frames: TRIO_DEBUG=1 ./src-tauri/target/release/bundle/macos/Trio.app/Contents/MacOS/trio"
