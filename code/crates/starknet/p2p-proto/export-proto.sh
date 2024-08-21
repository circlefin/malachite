#!/usr/bin/env bash

SCRIPT_PATH="$(dirname "$(realpath "$0")")"

ref="b7933a19ba1949369bf2a18d64e9a63c"
output="$SCRIPT_PATH/proto"

echo "Exporting proto files from 'buf.build/romac/starknet-p2p:$ref' to '$output'..."
buf export -o "$output" "buf.build/romac/starknet-p2p:$ref"
