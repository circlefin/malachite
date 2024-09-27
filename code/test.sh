#!/bin/bash

# Path to the node logs
LOG_PATH="$HOME/.malachite/*/logs/node.log"

while true; do
    echo "Generating configuration..."

    cargo run -- testnet --nodes 5 --enable-discovery

    echo "Starting spawn process..."

    # Start the spawn process in the background
    ./scripts/spawn.bash --nodes 5 --home ~/.malachite &
    SPAWN_PID=$!

    # Sleep for 5 seconds
    sleep 5

    # Send Ctrl-C (SIGINT) to terminate the spawn process
    echo "Stopping spawn process..."
    kill -SIGTERM $SPAWN_PID

    # Wait for the process to terminate fully
    wait $SPAWN_PID 2>/dev/null
    pkill -f target/release/malachite-cli

    # Check each log for the required line
    echo "Checking logs..."
    for log_file in $LOG_PATH; do
        if ! grep -q "Discovery finished" "$log_file"; then
            echo "Error: Required line not found in $log_file"
            exit 1
        fi
    done

    echo "All logs contain the required line. Repeating the process..."
done
