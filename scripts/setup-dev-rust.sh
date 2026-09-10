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
TOOLCHAIN_ROOT="$(dirname "$(dirname "$RUSTC")")"

# rust-lld expects libLLVM.dylib next to the host stdlib; rustup ships it one level up.
HOST_TRIPLE="$(rustc -vV | sed -n 's/^host: //p')"
LLVM_SRC="$TOOLCHAIN_ROOT/lib/libLLVM.dylib"
LLVM_DEST="$TOOLCHAIN_ROOT/lib/rustlib/$HOST_TRIPLE/lib/libLLVM.dylib"
if [[ -f "$LLVM_SRC" ]]; then
  mkdir -p "$(dirname "$LLVM_DEST")"
  if [[ ! -e "$LLVM_DEST" ]]; then
    ln -sf "$LLVM_SRC" "$LLVM_DEST"
    echo "Linked libLLVM.dylib for rust-lld -> $LLVM_DEST"
  fi
else
  echo "warning: $LLVM_SRC not found; try: rustup component add llvm-tools-preview"
fi

mkdir -p .cargo
cat > .cargo/config.toml <<EOF
# Homebrew Rust cannot compile wasm32-wasip2; point Cargo at rustup's rustc.
# Regenerate: ./scripts/setup-dev-rust.sh
[build]
rustc = "$RUSTC"
EOF

echo "Wrote .cargo/config.toml -> $RUSTC"
echo "Verify compile:  cargo check --target wasm32-wasip2"
echo "Verify link:     cargo build --target wasm32-wasip2"
