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

