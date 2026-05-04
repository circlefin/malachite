# Malachite Applications

This document describes the test application built on top of the Malachite BFT consensus engine.

## Overview

| | **Test App** |
|---|---|
| **Location** | `code/crates/test/app/` |
| **Purpose** | Integration testing, fault injection, and runnable example |
| **Integration level** | Channel-based (`app-channel`) |
| **Context** | `TestContext` (from `malachitebft-test`) |
| **Value type** | `u64` (factored into primes) |
| **Value payload mode** | `ProposalAndParts` (configurable) |
| **Network codec** | Protobuf |
| **Signing** | Ed25519 |

## Integration Level

The test app uses the **channel-based** integration level (`malachitebft-app-channel`). The engine is started with `EngineBuilder`, which returns a set of channels. The application runs a `tokio::spawn`ed loop over `channels.consensus.recv()`, handling `AppMsg` variants (`GetValue`, `ReceivedProposalPart`, `Decided`, `Finalized`, etc.). All actor wiring is handled internally.

## Test App (`code/crates/test/app/`)

### Purpose

Serves as both the integration test harness (used by `code/crates/test/tests/`) and a standalone runnable application with CLI support. Provides a middleware system for fault injection and configurable consensus behavior.

### Structure

| Module | File | Purpose |
|--------|------|---------|
| `main` | `src/main.rs` | CLI entry point (start, init, testnet, dump-wal) |
| `node` | `src/node.rs` | `App` (in-memory) and `CliApp` (file-based) `Node` impls |
| `app` | `src/app.rs` | Main `AppMsg` event loop, monitor_state, log level reloading |
| `state` | `src/state.rs` | Proposal creation, validation, value assembly, validator rotation |
| `config` | `src/config.rs` | Configuration loading with env var overlay, validator rotation config |
| `metrics` | `src/metrics.rs` | `DbMetrics` implementing `StoreMetrics` for Prometheus |
| `store` | `src/store.rs` | Re-export from `malachitebft-test-store` |
| `streaming` | `src/streaming.rs` | Re-export from `malachitebft-test-streaming` |

### Node Variants

- **`App`** — Used by the integration test framework. Configuration and keys are provided in-memory. Uses `NoMetrics` for the store.
- **`CliApp`** — Used by the CLI binary. Configuration and keys are loaded from files on disk. Registers Prometheus `DbMetrics` and serves a metrics endpoint.

### Context

Reuses `TestContext` from `malachitebft-test` (`crates/test/src/context.rs`). This context routes all operations through a `Middleware` trait, enabling per-node behavior customization in tests.

### Value Model

- Values are random `u64` integers
- Streamed as prime factors: `Init` (metadata) → `Data` (one per factor) → `Fin` (Keccak256 signature)
- 500ms artificial delay simulates execution

### Middleware System

The `Middleware` trait (`crates/test/src/middleware.rs`) provides hooks at key consensus points:

- `get_validator_set()` — override validator sets per height
- `get_timeouts()` — override timeouts
- `on_propose_value()` — intercept/modify proposals
- `get_validity()` — control validity decisions
- `on_commit()` — hook into commits

Built-in variants: `DefaultMiddleware`, `RotateValidators`, `EpochValidators`, `RotateEpochValidators`.

### Validator Set Resolution

The validator set for a given height is resolved with the following priority:

1. **Middleware** — if the middleware returns a validator set, it takes priority
2. **Validator rotation** — if `validator_rotation.enabled` is true in config, rotates the genesis validator set based on height and configured period/selection size
3. **Genesis** — falls back to the genesis validator set

### Configuration

The `Config` struct includes:
- Standard consensus, logging, metrics, runtime, and value sync configuration
- `test` — test-specific config (target time, etc.)
- `validator_rotation` — configurable validator set rotation (`enabled`, `rotation_period`, `selection_size`)

Configuration is loaded from TOML files with environment variable overlay (prefix `MALACHITE__`).

### Running

```bash
# Generate testnet configs
cargo run -p arc-malachitebft-test-app -- testnet --nodes 3 --home nodes

# Start a node
cargo run -p arc-malachitebft-test-app -- start --home nodes/node-0
```

### Test Scenarios

Integration tests cover: equivocation, finalization, full nodes, liveness, WAL recovery, value sync, Byzantine tolerance (n=3f+0, n=3f+1), pubsub protocols, and consensus modes.

## Shared Crates

### `malachitebft-test-streaming` (`code/crates/test/streaming/`)

Extracted streaming module for proposal part stream reassembly:

- `MinSeq<T>`, `MinHeap<T>` — MinHeap-based stream reassembly ensuring ordered delivery
- `StreamState` — per-stream buffer state
- `ProposalParts` — assembled proposal parts (height, round, proposer, parts vector)
- `PartStreamsMap` — multi-stream state machine managing concurrent proposal streams

### `malachitebft-test-store` (`code/crates/test/store/`)

Extracted redb-based persistent storage with pluggable metrics:

- `Store<M: StoreMetrics>` — async store backed by redb, generic over metrics implementation
- `Db<M>` — synchronous database layer with metrics instrumentation
- `DecidedValue` — committed value with its commit certificate
- `StoreMetrics` trait — pluggable metrics interface with default no-op implementations
- `NoMetrics` — zero-cost no-op metrics implementation
- Table management for decided values, certificates, undecided proposals, and pending proposal parts

## Crate Map

```
code/crates/test/                  # malachitebft-test (TestContext, Middleware, Node traits)
code/crates/test/streaming/        # malachitebft-test-streaming (stream reassembly)
code/crates/test/store/            # malachitebft-test-store (redb storage + metrics trait)
code/crates/test/app/              # malachitebft-test-app (lib + bin: test app with CLI)
code/crates/test/cli/              # malachitebft-test-cli (CLI arg parsing, logging, runtime)
code/crates/test/framework/        # malachitebft-test-framework (TestRunner, NodeInfo)
code/crates/test/tests/            # Integration tests
code/crates/test/mbt/              # Model-based tests
code/crates/test/mempool/          # Mempool utilities
```

## Dependency Graph

```
malachitebft-test-streaming
  └── malachitebft-app-channel, malachitebft-test

malachitebft-test-store
  └── malachitebft-test-streaming, malachitebft-app-channel, malachitebft-test

malachitebft-test-app (lib + bin)
  └── malachitebft-test-store, malachitebft-test-streaming
  └── malachitebft-test, malachitebft-test-cli
  └── malachitebft-app-channel
```
