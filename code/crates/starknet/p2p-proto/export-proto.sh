#!/usr/bin/env bash

SCRIPT_PATH="$(dirname "$(realpath "$0")")"

ref="72dda96ebb88492581b1bb2591fd2314"
output="$SCRIPT_PATH/proto"

echo "Exporting proto files from 'buf.build/romac/starknet-p2p:$ref' to '$output'..."
buf export -o "$output" "buf.build/romac/starknet-p2p:$ref"
