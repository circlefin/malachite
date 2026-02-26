#!/usr/bin/env bash

# fix-rebase-after-rename.sh
#
# Helper script for PR authors to run after rebasing onto main
# following the informalsystems- -> arc- crate rename.
#
# Usage:
#   git fetch origin main && git rebase origin/main
#   # resolve merge conflicts
#   ./scripts/fix-rebase-after-rename.sh
#   cargo check --workspace  # verify everything builds
#   git diff                 # review changes
#
# This script is idempotent and safe to run multiple times.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

echo "==> Replacing 'informalsystems-malachitebft-' with 'arc-malachitebft-' in config and build files..."

find "$REPO_ROOT/code" "$REPO_ROOT/.github" \
  -type f \( -name '*.toml' -o -name '*.yml' -o -name '*.yaml' -o -name '*.sh' -o -name '*.bash' \) \
  -not -path '*/target/*' \
  -exec sed -i '' 's/informalsystems-malachitebft-/arc-malachitebft-/g' {} +

# Also handle Makefile (no extension match)
find "$REPO_ROOT/code" -name 'Makefile' \
  -not -path '*/target/*' \
  -exec sed -i '' 's/informalsystems-malachitebft-/arc-malachitebft-/g' {} +

echo "==> Replacing 'informalsystems_malachitebft' with 'arc_malachitebft' in Rust and YAML files..."

find "$REPO_ROOT/code" "$REPO_ROOT/.github" \
  -type f \( -name '*.rs' -o -name '*.yml' -o -name '*.yaml' \) \
  -not -path '*/target/*' \
  -exec sed -i '' 's/informalsystems_malachitebft/arc_malachitebft/g' {} +

echo "==> Replacing 'informalsystems-malachitebft-' with 'arc-malachitebft-' in Rust string literals..."

find "$REPO_ROOT/code" \
  -type f -name '*.rs' \
  -not -path '*/target/*' \
  -exec sed -i '' 's/informalsystems-malachitebft-/arc-malachitebft-/g' {} +

echo "==> Replacing 'informalsystems-malachitebft-' with 'arc-malachitebft-' in documentation..."

find "$REPO_ROOT" -maxdepth 1 -type f -name '*.md' \
  -exec sed -i '' 's/informalsystems-malachitebft-/arc-malachitebft-/g' {} +

find "$REPO_ROOT/docs" -type f -name '*.md' \
  -exec sed -i '' 's/informalsystems-malachitebft-/arc-malachitebft-/g' {} + 2>/dev/null || true

find "$REPO_ROOT" -maxdepth 1 -type f -name '*.md' \
  -exec sed -i '' 's/informalsystems_malachitebft/arc_malachitebft/g' {} +

find "$REPO_ROOT/docs" -type f -name '*.md' \
  -exec sed -i '' 's/informalsystems_malachitebft/arc_malachitebft/g' {} + 2>/dev/null || true

echo "==> Regenerating Cargo.lock..."

cd "$REPO_ROOT/code"
cargo check --workspace 2>&1 | tail -5

echo ""
echo "==> Done! Please verify with:"
echo "    cargo check --workspace"
echo "    git diff"
echo ""
echo "Remaining 'informalsystems' references (should only be @informalsystems/quint, informalsystems/hermes, or historical URLs):"
grep -r 'informalsystems' "$REPO_ROOT/code" \
  --include='*.toml' --include='*.rs' --include='*.yml' --include='*.yaml' \
  --include='*.sh' --include='*.bash' \
  -l 2>/dev/null || echo "  (none found)"
