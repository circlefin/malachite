#!/usr/bin/env bash

# This script takes:
# - a number of nodes to run as an argument,
# - the home directory for the nodes configuration folders

function help {
    echo "Usage: spawn.sh [--help] --nodes NODES_COUNT --home NODES_HOME [--app APP_BINARY] [--no-reset]"
}

# Parse arguments
while [[ "$#" -gt 0 ]]; do
    case $1 in
        --help) help; exit 0 ;;
        --nodes) NODES_COUNT="$2"; shift ;;
        --home) NODES_HOME="$2"; shift ;;
        --app) APP_BINARY="$2"; shift ;;
        --no-reset) NO_RESET=1; shift ;;
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

if [[ -z "$APP_BINARY" ]]; then
    APP_BINARY="informalsystems_malachitebft_actor_app_proposal"
fi

# Environment variables
export MALACHITE__CONSENSUS__P2P__PROTOCOL__TYPE="gossipsub"
export MALACHITE__CONSENSUS__TIMEOUT_PROPOSE="3s"
export MALACHITE__CONSENSUS__TIMEOUT_PROPOSE_DELTA="500ms"
export MALACHITE__CONSENSUS__TIMEOUT_PREVOTE="1s"
export MALACHITE__CONSENSUS__TIMEOUT_PREVOTE_DELTA="500ms"
export MALACHITE__CONSENSUS__TIMEOUT_PRECOMMIT="1s"
export MALACHITE__CONSENSUS__TIMEOUT_PRECOMMIT_DELTA="500ms"
export MALACHITE__MEMPOOL__MAX_TX_COUNT="10000"
export MALACHITE__MEMPOOL__GOSSIP_BATCH_SIZE=0
export MALACHITE__MEMPOOL__LOAD_TYPE="uniform"
export MALACHITE__MEMPOOL__LOAD_UNIFORM__INTERVAL="500ms"
export MALACHITE__MEMPOOL__LOAD_UNIFORM__SIZE="256B"
export MALACHITE__MEMPOOL__LOAD_UNIFORM__COUNT="5000"
export MALACHITE__TEST__MAX_BLOCK_SIZE="128KiB"

export MALACHITE__TEST__VALUE_PAYLOAD="proposal-only"
export MALACHITE__TEST__TIME_ALLOWANCE_FACTOR=0.3
export MALACHITE__TEST__EXEC_TIME_PER_TX="0ms"
export MALACHITE__TEST__MAX_RETAIN_BLOCKS=10000
export MALACHITE__TEST__VOTE_EXTENSIONS__ENABLED="false"
export MALACHITE__VALUE_SYNC__ENABLED="true"
export MALACHITE__VALUE_SYNC__STATUS_UPDATE_INTERVAL="10s"
export MALACHITE__VALUE_SYNC__REQUEST_TIMEOUT="10s"

echo "Compiling '$APP_BINARY'..."
cargo build -p $APP_BINARY

# Create nodes and logs directories, run nodes
for NODE in $(seq 0 $((NODES_COUNT - 1))); do
    if [[ -z "$NO_RESET" ]]; then
        echo "[Node $NODE] Resetting the database..."
        rm -rf "$NODES_HOME/$NODE/db"
        mkdir -p "$NODES_HOME/$NODE/db"
        rm -rf "$NODES_HOME/$NODE/wal"
        mkdir -p "$NODES_HOME/$NODE/wal"
    fi

    rm -rf "$NODES_HOME/$NODE/logs"
    mkdir -p "$NODES_HOME/$NODE/logs"

    rm -rf "$NODES_HOME/$NODE/traces"
    mkdir -p "$NODES_HOME/$NODE/traces"

    echo "[Node $NODE] Spawning node..."
    cargo run -p $APP_BINARY -- start --home "$NODES_HOME/$NODE" > "$NODES_HOME/$NODE/logs/node.log" 2>&1 &
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
