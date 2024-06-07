#!/usr/bin/env fish

# This script takes:
# - a number of nodes to run as an argument, 
# - the home directory for the nodes configuration folders

function help
    echo "Usage: spawn.fish [--help] --nodes NODES_COUNT --home NODES_HOME"
end

argparse -n spawn.fish 'help' 'nodes=' 'home=' -- $argv
or return

if set -ql _flag_help
    help
    return 0
end

if ! set -q _flag_nodes
    help
    return 1
end

if ! set -q _flag_home
    help
    return 1
end

set -x MALACHITE__CONSENSUS__MAX_BLOCK_SIZE   "1 MiB"
set -x MALACHITE__TEST__TXS_PER_PART          50
set -x MALACHITE__TEST__TIME_ALLOWANCE_FACTOR 0.7
set -x MALACHITE__TEST__EXEC_TIME_PER_PART    "10ms"

set NODES_COUNT $_flag_nodes
set NODES_HOME  $_flag_home

for NODE in (seq 0 $(math $NODES_COUNT - 1))
  mkdir -p "$NODES_HOME/$NODE/logs"
  rm -f "$NODES_HOME/$NODE/logs/*.log"

  echo "[Node $NODE] Spawning node..."
  cargo run -q -- start --home "$NODES_HOME/$NODE" 2>&1 > "$NODES_HOME/$NODE/logs/node.log" &
  echo "$last_pid" > "$NODES_HOME/$NODE/node.pid"

  # set TAB_ID $(kitten @ launch --cwd=current --type=tab --tab-title "node-$NODE" fish)
  # sleep 0.5
  # kitten @ send-text --match-tab "id:$TAB_ID" "cargo run -q -- start --home \"$NODES_HOME/$NODE\" 2>&1 > \"$NODES_HOME/$NODE/logs/node.log\" &\n"
  # kitten @ send-text --match-tab "id:$TAB_ID" "echo \$last_pid > \"$NODES_HOME/$NODE/node.pid\"\n"
  # kitten @ send-text --match-tab "id:$TAB_ID" "tail -f \"$NODES_HOME/$NODE/logs/node.log\"\n"
end

function exit_and_cleanup --on-signal INT
    echo "Stopping all nodes..."
    for NODE in (seq 0 $(math $NODES_COUNT - 1))
        set NODE_PID (cat "$NODES_HOME/$NODE/node.pid")
        echo "[Node $NODE] Stopping node (PID: $NODE_PID)..."
        kill $NODE_PID
    end
    exit 0
end

echo "Spawned $NODES_COUNT nodes."
echo "Press Ctrl+C to stop the nodes."

while true; sleep 1; end

