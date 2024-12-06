#!/usr/bin/env bash

BLUE=$(tput setaf 4)
RED=$(tput setaf 1)
RESET=$(tput sgr0)

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

FILEPATH=$1
PROP=$2
MAX_STEPS=${3:-100}
[[ ! -f "$FILEPATH" ]] && error "file $FILEPATH does not exist" && exit 1
[[ -z "$PROP" ]] && error "property name required" && exit 1

MODULE=$(basename ${FILEPATH%".qnt"})
TRACES_DIR="traces/$MODULE"
mkdir -p "$TRACES_DIR"

# Given dir, name and ext, if "dir/name-N.ext" exists, it will return 
# "dir/name-M.ext" with M=N+1, otherwise it will return "dir/name-1.ext".
function nextFilename() {
    local dir=$1
    local name=$2
    local ext=$3
    local result="$dir/$name.$ext"
    if [ -f $result ]; then 
        i=1
        result="$dir/$name-$i.$ext"
        while [[ -e "$result" || -L "$result" ]] ; do
            result="$dir/$name-$((i+1)).$ext"
        done
    fi
    echo "$result"
}

TRACE_PATH=$(nextFilename "$TRACES_DIR" "$PROP" "itf.json")
OUTPUT=$(npx @informalsystems/quint run \
    --max-steps=$MAX_STEPS \
    --max-samples=1 \
    --invariant "$PROP" \
    --out-itf "$TRACE_PATH" \
    "$FILEPATH" 2>&1)
case $OUTPUT in
    "error: Invariant violated")
        # info "Success: reached a state that violates $FILEPATH::$PROP"
        info "Generated trace: $TRACE_PATH"
        ;;
    *)
        error "Failed: did not find a state that violates $FILEPATH::$PROP in $MAX_STEPS steps"
        [ -f $TRACE_PATH ] && info "Generated trace: $TRACE_PATH"
        ;;
esac
