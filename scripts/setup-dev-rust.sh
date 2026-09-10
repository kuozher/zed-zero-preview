#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if ! command -v rustup >/dev/null 2>&1; then
  echo "rustup not found. Install from https://rustup.rs and run: rustup target add wasm32-wasip2"
  exit 1
fi

rustup target add wasm32-wasip2 >/dev/null 2>&1 || true
RUSTC="$(rustup which rustc)"

mkdir -p .cargo
cat > .cargo/config.toml <<EOF
# Homebrew Rust cannot compile wasm32-wasip2; point Cargo at rustup's rustc.
# Regenerate: ./scripts/setup-dev-rust.sh
[build]
rustc = "$RUSTC"
EOF

echo "Wrote .cargo/config.toml -> $RUSTC"
echo "Verify: cargo check --target wasm32-wasip2"
