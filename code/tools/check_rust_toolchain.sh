#!/usr/bin/env bash
set -euo pipefail

# Simple helper to show the active Rust toolchain for the Malachite workspace.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
cd "${REPO_ROOT}"

echo "== Malachite Rust toolchain check =="
echo

if ! command -v rustc >/dev/null 2>&1; then
  echo "rustc is not installed. Please install Rust via rustup: https://rustup.rs/." >&2
  exit 1
fi

echo "rustc version:"
rustc --version
echo

if command -v cargo >/dev/null 2>&1; then
  echo "cargo version:"
  cargo --version
  echo
fi

if [ -f "rust-toolchain.toml" ] || [ -f "rust-toolchain" ]; then
  echo "Workspace toolchain file:"
  if [ -f "rust-toolchain.toml" ]; then
    cat "rust-toolchain.toml"
  else
    cat "rust-toolchain"
  fi
else
  echo "No rust-toolchain file found at repository root."
fi

echo
echo "Tip: run \"cargo test --all-features\" from the repo root to verify your setup."
