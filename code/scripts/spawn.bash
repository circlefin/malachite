#!/usr/bin/env bash

# This script takes:
# - a number of nodes to run as an argument,
# - the home directory for the nodes configuration folders

function help {
    echo "Usage: spawn.sh [--help] --nodes NODES_COUNT --home NODES_HOME"
}

# Parse arguments
while [[ "$#" -gt 0 ]]; do
    case $1 in
        --help) help; exit 0 ;;
        --nodes) NODES_COUNT="$2"; shift ;;
        --home) NODES_HOME="$2"; shift ;;
        *) echo "Unknown parameter passed: $1"; help; exit 1 ;;
    esac
    shift
done

# Check required arguments
if [[ -z "$NODES_COUNT" ]]; then
    help
    exit 1
fi

if [[ -z "$NODES_HOME" ]]; then
    help
    exit 1
fi

# Environment variables
export MALACHITE__CONSENSUS__P2P__PROTOCOL__TYPE="gossipsub"
export MALACHITE__CONSENSUS__MAX_BLOCK_SIZE="50KiB"
export MALACHITE__CONSENSUS__TIMEOUT_PROPOSE="5s"
export MALACHITE__CONSENSUS__TIMEOUT_PREVOTE="1s"
export MALACHITE__CONSENSUS__TIMEOUT_PRECOMMIT="1s"
export MALACHITE__CONSENSUS__TIMEOUT_COMMIT="0s"
export MALACHITE__MEMPOOL__MAX_TX_COUNT="10000"
export MALACHITE__MEMPOOL__GOSSIP_BATCH_SIZE=0
export MALACHITE__TEST__TX_SIZE="1KiB"
export MALACHITE__TEST__TXS_PER_PART=50
export MALACHITE__TEST__TIME_ALLOWANCE_FACTOR=0.5
export MALACHITE__TEST__EXEC_TIME_PER_TX="1ms"
export MALACHITE__TEST__VOTE_EXTENSIONS__ENABLED="true"
export MALACHITE__TEST__VOTE_EXTENSIONS__SIZE="1KiB"

echo "Compiling Malachite..."
cargo build --release

# Create nodes and logs directories, run nodes
for NODE in $(seq 0 $((NODES_COUNT - 1))); do
    mkdir -p "$NODES_HOME/$NODE/logs"
    rm -f "$NODES_HOME/$NODE/logs/*.log"

    echo "[Node $NODE] Spawning node..."
    cargo run -q --release -- start --home "$NODES_HOME/$NODE" > "$NODES_HOME/$NODE/logs/node.log" 2>&1 &
    echo $! > "$NODES_HOME/$NODE/node.pid"
done

# Function to handle cleanup on interrupt
function exit_and_cleanup {
    echo "Stopping all nodes..."
    for NODE in $(seq 0 $((NODES_COUNT - 1))); do
        NODE_PID=$(cat "$NODES_HOME/$NODE/node.pid")
        echo "[Node $NODE] Stopping node (PID: $NODE_PID)..."
        kill "$NODE_PID"
    done
    exit 0
}

# Trap the INT signal (Ctrl+C) to run the cleanup function
trap exit_and_cleanup INT

echo "Spawned $NODES_COUNT nodes."
echo "Press Ctrl+C to stop the nodes."

# Keep the script running
while true; do sleep 1; done
