install:
	npm i @informalsystems/quint -g
	cargo install --locked cargo-nextest

lint:
	cargo fmt --all --check
	cargo clippy --workspace --all-features --all-targets -- -D warnings

lint-fix:
	cargo clippy --fix --allow-dirty --allow-staged --workspace --all-features --all-targets -- -D warnings
	cargo fmt --all

unit-tests:
	cargo nextest run \
		--workspace \
		--all-features \
		--no-fail-fast \
		--failure-output final \
		--exclude informalsystems-malachitebft-starknet-test \
		--exclude informalsystems-malachitebft-discovery-test


test: unit-tests