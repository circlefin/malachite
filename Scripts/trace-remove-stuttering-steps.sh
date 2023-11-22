#!/usr/bin/env bash

FILE=$1
[[ ! -f "$FILE" ]] && echo "Error: $FILE does not exist" && exit 1

# The following jq command removes all stuttering steps on an ITF trace file. A stuttering step
# happens in a trace when a state is followed by the same state.
#
# Note that for comparing states we remove from all states the "#meta" field, which is different for
# each state as it includes a sequential index number. This information is not needed for displaying
# or doing MBT on the trace.
echo $(cat $1 | jq -c '.states |= [to_entries | .[] | select(.key % 2 == 0) | .value."#meta".index = .key / 2 | .value]') > $1
