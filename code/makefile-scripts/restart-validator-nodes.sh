#!/bin/bash

echo "Restarting validator nodes (0, 1, 2) while keeping node 3 running..."
echo ""


# Stop nodes 0, 1, 2 in parallel
echo "Stopping validator nodes..."
docker compose stop node0 node1 node2

echo "Waiting 2 seconds after stopping..."
sleep 2

echo "Socket status after stopping validators:"
echo "Socket connections:"
for node in node0 node1 node2 node3; do
	if docker compose ps $node 2>/dev/null | grep -q "Up"; then
		count=$(docker compose exec -T $node cat /proc/net/tcp 2>/dev/null | tail -n +2 | grep -E ":6978|:6979|:697A|:697B" | wc -l || echo "0")
		echo "  $node:       $count sockets"
	else
		echo "  $node:       DOWN"
	fi
done
echo ""

# Start all validator nodes in parallel
echo "Starting validator nodes..."
docker compose start node0 node1 node2

echo "Waiting for nodes to start..."
sleep 3

# Check startup status
for node in node0 node1 node2; do
    if docker compose ps $node | grep -q "Up"; then
        echo "$node started successfully"
    else
        echo "$node failed to start"
    fi
done

echo ""
echo "Socket status immediately after restart:"
echo "Socket connections:"
for node in node0 node1 node2 node3; do
	if docker compose ps $node 2>/dev/null | grep -q "Up"; then
		count=$(docker compose exec -T $node cat /proc/net/tcp 2>/dev/null | tail -n +2 | grep -E ":6978|:6979|:697A|:697B" | wc -l || echo "0")
		echo "  $node:       $count sockets"
	else
		echo "  $node:       DOWN"
	fi
done

echo "Validator nodes restart complete!"
echo ""
echo "Expected behavior:"
echo "  Nodes 0, 1, 2 are restarted (fresh containers)"
echo "  Node 3 remains running (preserves connections)"  
echo "  🔍 Node 3 should detect disconnections and attempt to reconnect"
echo "  🔍 Watch logs: docker compose logs -f node3"
echo ""
