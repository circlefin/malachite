#!/bin/bash

echo "Monitoring socket connections for socket leak detection..."
echo "Press Ctrl+C to stop"
echo

# Function to count sockets for a container
count_sockets() {
    local container_name="$1"
    local count=$(docker exec "$container_name" ss -t 2>/dev/null | grep :27000 | wc -l || echo "0")
    echo "$count"
}

# Function to get process info
get_process_info() {
    local container_name="$1"
    docker exec "$container_name" ps aux | head -2 2>/dev/null
}

# Monitor continuously
start_time=$(date +%s)
iteration=0

while true; do
    iteration=$((iteration + 1))
    current_time=$(date +%s)
    elapsed=$((current_time - start_time))
    
    echo "=== Check $iteration ($(date), ${elapsed}s elapsed) ==="
    
    # Check each container
    total_sockets=0
    for node in node0 node1 node2; do
        container_name="malachite_$node"
        socket_count=$(count_sockets "$container_name" 2>/dev/null || echo "N/A")
        
        if [ "$socket_count" != "N/A" ]; then
            total_sockets=$((total_sockets + socket_count))
        fi
        
        echo "  $container_name: $socket_count sockets"
    done
    
    echo "  Total sockets: $total_sockets"
    
    # Check for socket leak (arbitrary threshold)
    if [ "$total_sockets" -gt 50 ]; then
        echo "  POTENTIAL SOCKET LEAK DETECTED! ($total_sockets sockets)"
    elif [ "$total_sockets" -gt 20 ]; then
        echo "  WARNING: High socket count: $total_sockets"
    else
        echo "  Socket count normal"
    fi
    
    # Show recent log activity from node0
    echo "  Recent activity (node0):"
    docker logs malachite_node0 --tail 2 2>/dev/null | grep -E "(Reconnecting|Successfully initiated|Failed to dial)" | tail -1 | sed 's/^/    /'
    
    echo
    sleep 5
done
