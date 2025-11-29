#!/usr/bin/env bash
set -euo pipefail

echo "=== Malachite basic checks ==="
echo

# 1. Format
echo "[1/4] Running cargo fmt..."
cargo fmt --all
echo

# 2. Clippy
echo "[2/4] Running cargo clippy..."
cargo clippy --workspace --all-targets --all-features -- -D warnings
echo

# 3. Tests
echo "[3/4] Running cargo test..."
cargo test --workspace
echo

# 4. Optional Quint checks
echo "[4/4] Checking for Quint..."
if command -v quint >/dev/null 2>&1; then
  echo "Quint is available. Running a basic Quint command..."
  # Adjust this to match the project’s preferred Quint invocations.
  quint --help >/dev/null 2>&1 || true
  echo "Quint command completed (see project documentation for full Quint workflows)."
else
  echo "Quint is not installed or not in PATH."
  echo "Quint v0.22+ is recommended for working with the specifications."
fi

echo
echo "All basic checks completed."
echo "If any of the commands above failed, please fix the reported issues"
echo "before opening a pull request."
