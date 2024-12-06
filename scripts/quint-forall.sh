#!/bin/bash
#
# Run `quint` with the provided command on all provided files.
# Filenames are read from the standard input (stdin).
#

UNDERLINE=$(tput smul)
RESET=$(tput sgr0)

CMD=$@
if [ -z "$CMD" ] ; then
	echo "${UNDERLINE}Usage:${RESET} $0 <command>"
	exit 1
fi

# [INFO] message in blue
BLUE=$(tput setaf 4)
info() {
	echo "${BLUE}[INFO] $*${RESET}"
} 

# [ERROR] message in red
RED=$(tput setaf 1)
error() {
	echo "${RED}[ERROR] $*${RESET} "
}

FAILED=0
FAILED_FILES=()

# Read input files, one per line
while IFS="" read -r file; do
	info "Running: quint $CMD ${UNDERLINE}$file"
	if ! time npx @informalsystems/quint $CMD "$file"; then
		FAILED_FILES+=("$file")
		FAILED=$((FAILED + 1))
	fi
	echo ""
done

if [ "$FAILED" -gt 0 ]; then
	error "Failed on $FAILED files:"
	for file in "${FAILED_FILES[@]}"; do
		error " - ${UNDERLINE}$file"
	done
fi

exit $FAILED
