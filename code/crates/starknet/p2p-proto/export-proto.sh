#!/usr/bin/env bash

SCRIPT_PATH="$(dirname "$(realpath "$0")")"

ref="6eddedd9c77749ca929367fa81b53bc7"
output="$SCRIPT_PATH/proto"

echo "Exporting proto files from 'buf.build/romac/starknet-p2p:$ref' to '$output'..."
buf export -o "$output" "buf.build/romac/starknet-p2p:$ref"
