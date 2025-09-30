#!/bin/bash

set -e

echo "Setting up mixed-version testnet..."
echo "  Nodes 1-3 (node0,node1,node2): commit befe02ace90ea38eed4795d42157b3bfe61d0572 (old version)"
echo "  Node 4 (node3): latest commit (with socket leak fixes)"
echo ""

OLD_COMMIT="befe02ace90ea38eed4795d42157b3bfe61d0572"
CURRENT_COMMIT=$(git rev-parse HEAD)

echo "Current commit: $CURRENT_COMMIT"
echo "Old commit: $OLD_COMMIT"
echo ""

# Create target directories for different versions
mkdir -p target/mixed-testnet/old
mkdir -p target/mixed-testnet/latest

echo "Building old version binary (commit $OLD_COMMIT)..."
# Stash current changes if any
if ! git diff --quiet; then
    echo "💾 Stashing current changes..."
    git stash push -m "Mixed testnet setup - auto stash"
    STASHED=true
else
    STASHED=false
fi

# Build old version
git checkout $OLD_COMMIT
echo "Checked out old commit, building..."
RUST_MIN_STACK=16777216 cross build --release --target x86_64-unknown-linux-gnu
cp target/x86_64-unknown-linux-gnu/release/informalsystems-malachitebft-example-channel target/mixed-testnet/old/malachite

echo "Building latest version binary..."
# Return to latest
git checkout -
if [ "$STASHED" = true ]; then
    echo "Restoring stashed changes..."
    git stash pop
fi

# Build latest version
RUST_MIN_STACK=16777216 cross build --release --target x86_64-unknown-linux-gnu
cp target/x86_64-unknown-linux-gnu/release/informalsystems-malachitebft-example-channel target/mixed-testnet/latest/malachite

echo ""
echo " Creating mixed-version docker compose.yml..."

# Create the mixed-version docker compose file
cat > docker-compose-mixed.yml << 'EOF'
networks:
  testnet:
    driver: bridge
    ipam:
      config:
        - subnet: 172.20.0.0/16

services:
  node0:
    image: ubuntu:22.04
    platform: linux/amd64
    container_name: malachite_node0
    networks:
      testnet:
        ipv4_address: 172.20.0.10
    volumes:
      - ./target/mixed-testnet/old/malachite:/usr/local/bin/malachite:ro
      - ./deployments/volumes/malachite/0:/app/node
    ports:
      - "29000:29000"
      - "27000:27000"
    environment:
      - RUST_LOG=debug,libp2p_swarm::handler=off,libp2p_swarm=info
    command: /usr/local/bin/malachite start --home /app/node
    restart: unless-stopped

  node1:
    image: ubuntu:22.04
    platform: linux/amd64
    container_name: malachite_node1
    networks:
      testnet:
        ipv4_address: 172.20.0.11
    volumes:
      - ./target/mixed-testnet/old/malachite:/usr/local/bin/malachite:ro
      - ./deployments/volumes/malachite/1:/app/node
    ports:
      - "29001:29000"
      - "27001:27000"
    environment:
      - RUST_LOG=debug,libp2p_swarm::handler=off,libp2p_swarm=info
    command: /usr/local/bin/malachite start --home /app/node
    restart: unless-stopped

  node2:
    image: ubuntu:22.04
    platform: linux/amd64
    container_name: malachite_node2
    networks:
      testnet:
        ipv4_address: 172.20.0.12
    volumes:
      - ./target/mixed-testnet/old/malachite:/usr/local/bin/malachite:ro
      - ./deployments/volumes/malachite/2:/app/node
    ports:
      - "29002:29000"
      - "27002:27000"
    environment:
      - RUST_LOG=debug,libp2p_swarm::handler=off,libp2p_swarm=info
    command: /usr/local/bin/malachite start --home /app/node
    restart: unless-stopped

  node3:
    image: ubuntu:22.04
    platform: linux/amd64
    container_name: malachite_node3
    networks:
      testnet:
        ipv4_address: 172.20.0.13
    volumes:
      - ./target/mixed-testnet/latest/malachite:/usr/local/bin/malachite:ro
      - ./deployments/volumes/malachite/3:/app/node
    ports:
      - "29003:29000"
      - "27003:27000"
    environment:
      - RUST_LOG=debug,libp2p_swarm::handler=off,libp2p_swarm=info
    command: /usr/local/bin/malachite start --home /app/node
    restart: unless-stopped
EOF
