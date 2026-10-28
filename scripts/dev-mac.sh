#!/usr/bin/env bash
# Run Trio in development on macOS.
set -euo pipefail

cd "$(dirname "$0")/../src-tauri"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "Trio's gesture engine only runs on macOS (this is $(uname -s))." >&2
  echo "You can still run 'cargo check' and 'cargo test' here." >&2
fi

command -v cargo >/dev/null 2>&1 || {
  echo "cargo not found. Install Rust: https://rustup.rs" >&2
  exit 1
}

if ! cargo tauri --version >/dev/null 2>&1; then
  echo "Tauri CLI not found. Install it with:" >&2
  echo "  cargo install tauri-cli --version '^2' --locked" >&2
  echo "or run 'npx tauri dev' from the project root." >&2
  exit 1
fi

exec cargo tauri dev "$@"
