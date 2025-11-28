#!/usr/bin/env bash
set -euo pipefail

REQUIRED_RUST="1.82.0"

echo "=== Malachite development environment check ==="

# Check rustc
if ! command -v rustc >/dev/null 2>&1; then
  echo "Error: rustc is not installed or not in PATH."
  echo "Please install Rust from https://rustup.rs/ (Rust ${REQUIRED_RUST} or newer is required)."
  exit 1
fi

RUST_VERSION="$(rustc --version | awk '{print $2}')"
echo "Detected rustc version: ${RUST_VERSION}"

# Version comparison using sort -V (available on most Unix-like systems)
if [ "$(printf '%s\n%s\n' "${REQUIRED_RUST}" "${RUST_VERSION}" | sort -V | head -n1)" != "${REQUIRED_RUST}" ]; then
  echo "Warning: rustc ${RUST_VERSION} is older than the recommended ${REQUIRED_RUST}."
  echo "Some features or tests may not work as expected."
else
  echo "OK: rustc version meets the recommended minimum (${REQUIRED_RUST})."
fi

echo

# Check Quint
if command -v quint >/dev/null 2>&1; then
  QUINT_VERSION_OUTPUT="$(quint --version 2>/dev/null || true)"
  echo "Quint appears to be installed."
  if [ -n "${QUINT_VERSION_OUTPUT}" ]; then
    echo "quint --version output: ${QUINT_VERSION_OUTPUT}"
  fi
else
  echo "Note: Quint does not appear to be installed or is not in PATH."
  echo "Quint v0.22+ is recommended for working with the specifications and tests."
fi

echo
echo "Environment check completed."
