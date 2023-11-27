#!/usr/bin/env bash

BLUE=$(tput setaf 4)
RED=$(tput setaf 1)
RESET=$(tput sgr0)
UNDERLINE=$(tput smul)

# [INFO] message in blue
info()
{
  echo "${BLUE}[INFO] $*${RESET}"
} 

# [ERROR] message in red
error()
{
  echo "${RED}[ERROR] $*${RESET} "
}

# Run `quint $command` on all given files.

cmd="$1"
files=("${@:2}")

if [[ "${#files[@]}" -eq 0 ]]; then
  echo "${UNDERLINE}Usage:${RESET} $0 <command> <file> [<file> ...]"
  exit 1
fi

failed=0
failed_files=()

for file in "${files[@]}"; do
  info "Running: quint $cmd ${UNDERLINE}$file"
  if ! npx @informalsystems/quint "$cmd" "$file"; then
    failed_files+=("$file")
    failed=$((failed + 1))
  fi
  echo ""
done

if [[ "$failed" -gt 0 ]]; then
  error "Failed on $failed files:"
  for file in "${failed_files[@]}"; do
    error " - ${UNDERLINE}$file"
  done
  exit 1
fi
