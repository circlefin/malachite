#!/usr/bin/env bash

FILEPATH=$1
PROP=$2
MAX_STEPS=${3:-100}
[[ ! -f "$FILEPATH" ]] && echo "Error: file $FILEPATH does not exist" && exit 1
[[ -z "$PROP" ]] && echo "Error: property name required" && exit 1

MODULE=$(basename ${FILEPATH%".qnt"})
TRACES_DIR="traces/$MODULE"
mkdir -p "$TRACES_DIR"

# Given dir, name and ext, if "dir/name-N.ext" exists, it will return 
# "dir/name-M.ext" with M=N+1, otherwise it will return "dir/name-1.ext".
function nextFilename() {
    local dir=$1
    local name=$2
    local ext=$3
    i=1
    while [[ -e "$dir/$name-$i.$ext" || -L "$dir/$name-$i.$ext" ]] ; do
        let i++
    done
    name=$name-$i
    echo $name.$ext
}

FILE_NAME=$(nextFilename "$TRACES_DIR" "$PROP" "itf.json")
TRACE_PATH="$TRACES_DIR/$FILE_NAME"
# echo "Generating $MAX_STEPS $TRACE_PATH for $FILEPATH::$PROP..."

OUTPUT=$(npx @informalsystems/quint run \
    --max-steps=$MAX_STEPS \
    --max-samples=1 \
    --invariant "$PROP" \
    --out-itf "$TRACE_PATH" \
    "$FILEPATH" 2>&1)
case $OUTPUT in
    "error: Invariant violated")
        echo "Generated trace: $TRACE_PATH"
        echo "Success: reached a state that violates $FILEPATH::$PROP"
        ;;
    *)
        [ -f $TRACE_PATH ] && echo "Generated trace: $TRACE_PATH"
        echo "Failed: did not find a state that violates $FILEPATH::$PROP in $MAX_STEPS steps"
        ;;
esac
