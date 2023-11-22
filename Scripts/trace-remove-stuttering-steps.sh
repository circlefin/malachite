#!/usr/bin/env bash

FILE=$1
[[ ! -f "$FILE" ]] && echo "Error: $FILE does not exist" && exit 1

# The following jq command removes all stuttering steps on an ITF trace file. A stuttering step
# happens in a trace when a state is followed by the same state.
#
# Note that for comparing states we remove from all states the "#meta" field, which is different for
# each state as it includes a sequential index number. This information is not needed for displaying
# or doing MBT on the trace.
#
# TODO: For completeness, include the state #meta field in the resulting trace file.
echo $(cat $1 | jq -c '{"#meta":."#meta", vars:.vars, states: (.states | del(.[]."#meta") | reduce .[] as $a ([]; if IN(.[]; $a) then . else . += [$a] end))}') > $1
