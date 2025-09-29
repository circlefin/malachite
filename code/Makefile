##@ Development Tools

.PHONY: help install lint lint-fix integration-tests starknet-tests discovery-tests tests

help: ## Show this help.
	@awk 'BEGIN {FS = ":.*##"; printf "\nUsage: make \033[36m\033[0m\n"} /^[$$()% a-zA-Z_-]+:.*?##/ { printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2 } /^##@/ { printf "\n\033[1m%s\033[0m\n", substr($$0, 5) } ' $(MAKEFILE_LIST)

install: ## Install the required tools.
	npm i @informalsystems/quint -g
	cargo install --locked cargo-nextest

lint: ## Lint the workspace.
	cargo fmt --all --check
	cargo clippy --workspace --all-features --all-targets -- -D warnings

lint-fix: ## Fix the linting issues.
	cargo clippy --fix --allow-dirty --allow-staged --workspace --all-features --all-targets -- -D warnings
	cargo fmt --all

integration-tests: ## Run the integration tests.
	cargo nextest run \
		--workspace \
		--all-features \
		--no-fail-fast \
		--failure-output final \
		--test-threads 1 \
		--exclude informalsystems-malachitebft-starknet-test \
		--exclude informalsystems-malachitebft-discovery-test

starknet-tests: ## Run the Starknet integration tests
	cargo nextest run \
		--all-features \
		--no-fail-fast \
		--failure-output final \
		--test-threads 1 \
		--package informalsystems-malachitebft-starknet-test

discovery-tests: ## Run the discovery tests
	cargo nextest run \
		--all-features \
		--no-fail-fast \
		--failure-output final \
		--test-threads 1 \
		--package informalsystems-malachitebft-discovery-test

tests: ## Run all the tests.
	$(MAKE) integration-tests
	$(MAKE) starknet-tests
	$(MAKE) discovery-tests

##@ Testnet

.PHONY: testnet testnet-mixed testnet-build testnet-down testnet-clean restart-validators test-integration

testnet: testnet-clean ## Start testnet with 4 malachite containers (single network)
	@echo "Setting up single-network testnet..."
	@echo "Creating deployment directories..."
	@mkdir -p ./deployments/volumes/malachite

	@echo "Generating base testnet structure (genesis, keys, etc.)..."
	@cargo run --bin informalsystems-malachitebft-example-channel -- \
		testnet \
		--nodes 4 \
		--home ./deployments/volumes/malachite

	@echo "Copying single-network configs..."
	@cp ./single-network-configs/node0-config.toml ./deployments/volumes/malachite/0/config/config.toml
	@cp ./single-network-configs/node1-config.toml ./deployments/volumes/malachite/1/config/config.toml
	@cp ./single-network-configs/node2-config.toml ./deployments/volumes/malachite/2/config/config.toml
	@cp ./single-network-configs/node3-config.toml ./deployments/volumes/malachite/3/config/config.toml

	@echo "Starting single-network testnet..."
	@docker compose up -d

	@echo "Single-network testnet started!"
	@echo "  Network topology:"
	@echo "    All nodes: Single network (172.20.0.x)"
	@echo "    Node3: Connects via host IP (192.168.1.147) to test port mapping scenario"

testnet-mixed: testnet-clean ## Start mixed-version testnet with old and new binaries
	@echo "Setting up mixed-version testnet..."
	@echo "Creating deployment directories..."
	@mkdir -p ./deployments/volumes/malachite

	@echo "Generating base testnet structure (genesis, keys, etc.)..."
	@cargo run --bin informalsystems-malachitebft-example-channel -- \
		testnet \
		--nodes 4 \
		--home ./deployments/volumes/malachite

	@echo "Copying single-network configs..."
	@cp ./single-net-configs/node0-config.toml ./deployments/volumes/malachite/0/config/config.toml
	@cp ./single-net-configs/node1-config.toml ./deployments/volumes/malachite/1/config/config.toml
	@cp ./single-net-configs/node2-config.toml ./deployments/volumes/malachite/2/config/config.toml
	@cp ./single-net-configs/node3-config.toml ./deployments/volumes/malachite/3/config/config.toml

	@echo "Building mixed-version binaries..."
	@./makefile-scripts/setup-mixed-testnet.sh

	@echo "Starting mixed-version testnet..."
	@docker compose -f docker-compose-mixed.yml up -d

	@echo "Mixed-version testnet started!"
	@echo "  Network topology: Same as single-network testnet"
	@echo "  Nodes 0,1,2: Old version binaries"
	@echo "  Node 3: Latest version (current HEAD)"

testnet-build: ## Build the testnet binary and Docker images
	@echo "Building Linux binary for containers..."
	@RUST_MIN_STACK=16777216 cross build --release --target x86_64-unknown-linux-gnu --bin informalsystems-malachitebft-example-channel
	@echo "Building Docker images..."
	@docker compose build

testnet-down: ## Stop testnet
	@docker compose down > /dev/null 2>&1 || true

testnet-clean: testnet-down ## Remove testnet artifacts
	@rm -rf ./deployments/volumes/
	@rm -rf ./docker-nodes/
	@rm -rf ./test-nodes/
	@rm -f node_*.log

restart-validators: ## Restart validator nodes (0,1,2) while keeping node 3 running
	@./makefile-scripts/restart-validator-nodes.sh

test-integration: ## Full integration test: start testnet, check sockets, restart validators, check sockets
	@echo "Starting comprehensive testnet test..."
	@echo ""
	@echo "Test:"
	@echo "  1. Start testnet (4 nodes)"
	@echo "  2. Wait for initial connections (15s)"
	@echo "  3. Check socket status"
	@echo "  4. Restart validators (nodes 0,1,2)"
	@echo "  5. Wait for reconnections (10s)"
	@echo "  6. Final socket status check"
	@echo ""
	@$(MAKE) testnet
	@echo ""
	@echo "Waiting 15 seconds for initial connections..."
	@sleep 15
	@echo ""
	@echo "Initial Socket Status:"
	@$(MAKE) status
	@echo ""
	@echo "Restarting validators..."
	@$(MAKE) restart-validators
	@echo ""
	@echo "Waiting 10 seconds for reconnections..."
	@sleep 10
	@echo ""
	@echo "Final Socket Status:"
	@$(MAKE) status
	@echo ""
	@echo "Test complete! Check socket counts before/after restart."

##@ Debug Commands

.PHONY: monitor-sockets logs logs-node3 status

monitor-sockets: ## Monitor socket connections in real-time
	@./makefile-scripts/check-socket-leaks-simple.sh monitor

logs: ## Show logs from all nodes
	@docker compose logs node0 node1 node2 node3

logs-node3: ## Follow logs from node 3 (useful after validator restart)
	@docker compose logs -f node3

status: ## Show testnet status
	@echo "Socket connections:"
	@for node in node0 node1 node2 node3; do \
		if docker compose ps $$node 2>/dev/null | grep -q "Up"; then \
			count=$$(docker compose exec -T $$node cat /proc/net/tcp 2>/dev/null | tail -n +2 | grep -E ":6978|:6979|:697A|:697B" | wc -l || echo "0"); \
			echo "  $$node: $$count sockets"; \
		else \
			echo "  $$node: DOWN"; \
		fi; \
	done

testnet-multi: testnet-clean ## Start multi-network testnet to test NAT/address scenarios
	@echo "Setting up multi-network testnet..."
	@echo "Creating deployment directories..."
	@mkdir -p ./deployments/volumes/malachite

	@echo "Generating base testnet structure (genesis, keys, etc.)..."
	@cargo run --bin informalsystems-malachitebft-example-channel -- \
		testnet \
		--nodes 4 \
		--home ./deployments/volumes/malachite

	@echo "Copying multi-network configs..."
	@cp ./multi-net-configs/node0-config.toml ./deployments/volumes/malachite/0/config/config.toml
	@cp ./multi-net-configs/node1-config.toml ./deployments/volumes/malachite/1/config/config.toml
	@cp ./multi-net-configs/node2-config.toml ./deployments/volumes/malachite/2/config/config.toml
	@cp ./multi-net-configs/node3-config.toml ./deployments/volumes/malachite/3/config/config.toml

	@echo "Starting multi-network testnet..."
	@docker compose -f docker-compose-multi-network.yml up -d

	@echo "Multi-network testnet started!"
	@echo "  Network topology:"
	@echo "    Validators (0,1,2): Internal (172.21.0.x) + Public (172.23.0.x) networks"
	@echo "    Node3: External (172.22.0.x) + Public (172.23.0.x) networks"
	@echo "  Test scenario: Address mismatch between dial_data and identify info"
	@echo "    - Node3 dials validators via 172.23.0.x (public addresses)"
	@echo "    - Validators advertise 172.21.0.x (internal addresses) in identify"
	@echo "    - Bootstrap matching tests dial_data vs identify address logic"

testnet-nat: testnet-clean ## Start NAT testnet scenario
	@echo "Setting up NAT testnet scenario..."
	@echo "Creating deployment directories..."
	@mkdir -p ./deployments/volumes/malachite

	@echo "Generating base testnet structure (genesis, keys, etc.)..."
	@cargo run --bin informalsystems-malachitebft-example-channel -- \
		testnet \
		--nodes 4 \
		--home ./deployments/volumes/malachite

	@echo "Copying NAT scenario configs..."
	@cp ./nat-configs/node0-config.toml ./deployments/volumes/malachite/0/config/config.toml
	@cp ./nat-configs/node1-config.toml ./deployments/volumes/malachite/1/config/config.toml
	@cp ./nat-configs/node2-config.toml ./deployments/volumes/malachite/2/config/config.toml
	@cp ./nat-configs/node3-config.toml ./deployments/volumes/malachite/3/config/config.toml

	@echo "Starting NAT testnet..."
	@docker compose -f docker-compose-nat.yml up -d

	@echo "NAT testnet started!"
	@echo "  Network topology:"
	@echo "    Validators (0,1,2): Internal network (192.168.100.x) with port mappings"
	@echo "    Node3: External network (10.0.1.x) reaching validators via NAT gateway"
	@echo "  Test scenario: NAT traversal and bootstrap identification"
	@echo "    - Node3 connects via host IP (10.0.1.1) to reach validators"
	@echo "    - Validators have internal IPs but are reachable via port mapping"
