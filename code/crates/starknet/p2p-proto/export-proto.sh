#!/usr/bin/env bash

SCRIPT_PATH="$(dirname "$(realpath "$0")")"

ref="v0.1.0"
output="$SCRIPT_PATH/proto"

echo "Exporting proto files from 'buf.build/romac/starknet-p2p:$ref' to '$output'..."
buf export -o "$output" "buf.build/romac/starknet-p2p:$ref"
