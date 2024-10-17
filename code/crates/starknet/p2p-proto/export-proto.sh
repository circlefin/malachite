#!/usr/bin/env bash

SCRIPT_PATH="$(dirname "$(realpath "$0")")"

ref="0a94cdbd3387478c94c9f306a095703e"
output="$SCRIPT_PATH/proto"

echo "Exporting proto files from 'buf.build/romac/starknet-p2p:$ref' to '$output'..."
buf export -o "$output" "buf.build/romac/starknet-p2p:$ref"
