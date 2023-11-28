#!/usr/bin/env bash

FILE=$1
[[ ! -f "$FILE" ]] && echo "Error: $FILE does not exist" && exit 1

# The following jq command removes all even steps on an ITF trace file. This is a hack to get round
# a Quint issue which makes the generated trace to have a stuttering step on `run` steps that are
# used for checking assertions, where all variables are unchanged (see
# https://github.com/informalsystems/quint/issues/1252).
echo $(cat $1 | jq -c '.states |= [to_entries | .[] | select(.key % 2 == 0) | .value."#meta".index = .key / 2 | .value]') > $1
