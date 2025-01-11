fmt:
	cd code && cargo +nightly fmt

lint:
	cd code && cargo fmt --all --check

lint-fix:
	make fmt
	cd code && cargo clippy --fix --allow-dirty --workspace -- -D warnings

test:
	cd code && cargo test