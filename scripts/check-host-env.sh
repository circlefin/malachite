#!/usr/bin/env bash
set -euo pipefail

echo "=== Malachite host environment check ==="
echo

# 1. Rust toolchain
echo "[1/4] Rust toolchain"
if command -v rustc >/dev/null 2>&1; then
  rustc --version || echo "  - rustc is installed but version check failed."
else
  echo "  - rustc not found on PATH."
fi
echo

# 2. CPU information
echo "[2/4] CPU information"

if command -v nproc >/dev/null 2>&1; then
  CPUS="$(nproc)"
  echo "  - Detected CPUs (nproc): ${CPUS}"
elif [ -f /proc/cpuinfo ]; then
  CPUS="$(grep -c '^processor' /proc/cpuinfo || echo 0)"
  echo "  - Detected CPUs (/proc/cpuinfo): ${CPUS}"
else
  echo "  - Could not determine CPU count."
fi
echo

# 3. Memory information
echo "[3/4] Memory information"

if command -v free >/dev/null 2>&1; then
  echo "  - Output of 'free -h':"
  free -h
elif [ -f /proc/meminfo ]; then
  echo "  - Excerpt from /proc/meminfo:"
  grep -E 'MemTotal|MemFree|MemAvailable' /proc/meminfo || true
else
  echo "  - Could not determine memory information."
fi
echo

# 4. Basic OS information
echo "[4/4] Operating system"

if command -v uname >/dev/null 2>&1; then
  echo "  - uname -a:"
  uname -a
else
  echo "  - uname not found on PATH."
fi

echo
echo "Host environment check complete."
echo "Use this information as a quick sanity check that the machine is"
echo "appropriate for running Malachite and its application workloads."
