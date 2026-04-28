# AGENTS.md

This file provides guidance to AI agents when working with code in this repository.

## Project Overview

Malachite is a Byzantine Fault Tolerant (BFT) consensus engine implementing the Tendermint consensus algorithm in Rust. It is a Cargo workspace rooted at `code/Cargo.toml` (not the repo root).

## Build and Test Commands

All commands run from the `code/` directory:

```bash
cargo build                    # Build the workspace
cargo lint                     # Clippy with all features
cargo fmt --check              # Check formatting
cargo nextest run -p <CRATE>   # Run tests for a specific crate
cargo nextest run -p <CRATE> -E 'test(test_name)'  # Run a single test
cargo all-tests                # All tests except MBT
cargo mbt                      # Model-based tests (requires Quint)
```

Integration test packages:
- `arc-malachitebft-test` — test app integration tests
- `arc-malachitebft-discovery-test` — discovery protocol tests

Note: Published crate names use the `arc-malachitebft-*` prefix, but workspace dependency aliases use `malachitebft-*` (e.g., `malachitebft-engine`). When specifying `-p` to cargo, use the published name.

## Architecture

### Core Design: Suspended Effects Pattern

Malachite's consensus logic is **pure and stateless** — it performs no I/O. Instead, it uses a coroutine-based effect system:

1. The `Context` trait (`core-types`) parameterizes all consensus types (Address, Height, Value, Vote, Proposal, ValidatorSet, SigningScheme, etc.)
2. The consensus `handle()` function is a coroutine (via `genawaiter`) that yields **Effects** (sign, publish, decide, WAL append, etc.) and receives **Resume** values back
3. The `process!` macro (`core-consensus/src/macros.rs`) drives this loop: feed an Input → run coroutine → handle yielded Effect → resume with result → repeat
4. **Actors** (via `ractor`) handle effects asynchronously in the engine layer

### Crate Organization

**Core consensus** (pure, no I/O, `no_std`-friendly):
- `core-types` — `Context` trait and all type traits (Value, Vote, Proposal, etc.)
- `core-state-machine` — single-round Tendermint state machine
- `core-votekeeper` — vote aggregation and quorum tracking
- `core-driver` — multi-round orchestration
- `core-consensus` — effect/input system, `process!` macro, top-level handlers

**Engine layer** (async, actor-based):
- `engine` — actor wiring: Node (supervisor), Consensus, Host, Network, WAL, Sync actors
- `app` / `app-channel` — high-level channel-based application interface
- `network` — libp2p-based P2P layer
- `discovery` — peer discovery protocol
- `sync` — block synchronization protocol
- `wal` — write-ahead log for crash recovery
- `config` / `metrics` / `codec` / `proto` / `peer` / `signing` / `signing-ed25519` / `signing-ecdsa`

**Test:**
- `test/` — integration test app, framework, MBT, CLI, mempool utilities

### Three Integration Levels

1. **Channel-based** (`app-channel`) — highest level, batteries-included (networking, sync, WAL)
2. **Actor-based** (`engine`) — swap individual actors (e.g., custom network layer)
3. **Core library** (`core-consensus`) — fully agnostic, bring your own everything

### Key Patterns

- The `perform!` macro inside effect handlers yields an effect and destructures the resume value in one expression
- `ConsensusState<Ctx>` holds the `Driver<Ctx>` (round state machine), input queue, proposal keeper, and timing metadata
- Message flow: Network/Host/Timer → Consensus Actor → `process!(input, state, on: effect)` → Effect Handler routes to other actors → Resume value fed back

## Conventions

- Commit messages: conventional commits with Jira references, e.g., `feat: Discovery peers request rate limiter`
- Rust edition 2021, MSRV 1.88
- Workspace lints: `clippy::disallowed_types = "deny"` — check `code/clippy.toml` for the disallowed types list

## CI Checks and PR Workflow

### CI checks on pull requests

All checks run automatically on PRs. Checks under `rust.yml` and `quint.yml` are skipped when their respective directories (`code/`, `specs/`) have no changes.

| Workflow | Jobs | Triggers on |
|----------|------|-------------|
| **Rust** (`rust.yml`) | Unit tests, Integration tests (discovery, test app), Clippy, Formatting, no_std compatibility, MSRV, Standalone feature checks | `code/` changes |
| **Quint** (`quint.yml`) | Typecheck, Test | `specs/` changes |
| **Semver** (`semver.yml`) | Detect semver violations (posts PR comment if breaking) | `code/` changes |
| **Coverage** (`coverage.yml`) | Code coverage report (posted as PR comment) | `code/` or `specs/` changes |
| **Spelling** (`typos.yml`) | Typo check via `typos` | Always |
| **PR** (`pr.yml`) | PR title lint (conventional commits) | Always |
| **AI PR** (`ai-pr.yml`) | Automated Claude Code review on open/sync; PR notes on `ai-pr-notes` label | Always |

### Before opening a PR

Run these from the `code/` directory to catch issues locally:

```bash
cargo fmt --all              # Fix formatting
cargo lint                   # Clippy with all features
cargo all-tests              # All tests except MBT
```

If you changed specs, also run from the repo root:

```bash
cargo mbt                    # Model-based tests (requires Quint)
```

PR titles must follow conventional commits (e.g., `feat: Add feature`). The `pr.yml` check enforces this.

## Architecture Decision Records

ADRs in `docs/architecture/` document key design decisions. Consult these when working on the relevant subsystems:

- [ADR-001](docs/architecture/adr-001-architecture.md) — **High-level architecture**: repository layout, component overview (networking, host, consensus driver, state machine, vote keeper), and how they compose
- [ADR-002](docs/architecture/adr-002-node-actor.md) — **Actor system design**: why ractor was chosen, actor decomposition (Consensus, Gossip, Mempool, WAL, Persistence, Timers, Host), and message flow between actors
- [ADR-003](docs/architecture/adr-003-values-propagation.md) — **Value propagation modes**: three modes for disseminating proposed values (`ProposalOnly`, `PartsOnly`, `ProposalAndParts`) and when to use each
- [ADR-004](docs/architecture/adr-004-coroutine-effect-system.md) — **Coroutine-based effect system**: the `process!`/`perform!` macro design, why coroutines were chosen over callbacks/traits/message-passing, and the Effect→Resume contract
- [ADR-005](docs/architecture/adr-005-value-sync.md) — **Value sync protocol**: how nodes catch up on decided values, peer management, request/response protocol, and integration with the consensus engine
- [ADR-006](docs/architecture/adr-006-proof-of-validator.md) — **Proof-of-Validator protocol**: cryptographic validator identity proofs for connection prioritization and mesh optimization
- [ADR-007](docs/architecture/adr-007-write-ahead-log.md) — **Write-Ahead Log (WAL)**: crash-recovery model, what gets logged (votes, proposals, timeouts), replay on restart to ensure consistency

## Specs

Formal specifications live in `specs/` using Quint (a TLA+-like language). Subdirectories cover consensus, network, block-streaming, and synchronization protocols.

### English specifications for consensus

Alongside the executable Quint spec, `specs/consensus/` contains English documents describing the Tendermint protocol and Malachite's implementation. Consult these when reasoning about protocol semantics, state transitions, or Byzantine behavior:

- [specs/consensus/README.md](specs/consensus/README.md) — **Entry point**: index and orientation for the consensus specification, with references to the Tendermint paper
- [specs/consensus/overview.md](specs/consensus/overview.md) — **Protocol overview**: summary of Tendermint at the protocol level — rounds, steps (propose/prevote/precommit), locking, and safety/liveness assumptions — as well as the discussion of the practical requirements not discussed in the paper — proposer selection, validity predicate, network properties, etc.
- [specs/consensus/pseudo-code.md](specs/consensus/pseudo-code.md) — **Algorithm pseudo-code**: Algorithm 1 from page 6 of the Tendermint paper, copied verbatim for easy cross-reference from the rest of the spec and the code
- [specs/consensus/design.md](specs/consensus/design.md) — **Malachite design**: how the implementation separates vote keeper, driver, and state machine; principles underlying the consensus state machine
- [specs/consensus/message-handling.md](specs/consensus/message-handling.md) — **Message handling**: discussion on how processes should handle messages for rounds and heights different from their current one (future/past rounds, lagging/leading heights)
- [specs/consensus/misbehavior.md](specs/consensus/misbehavior.md) — **Byzantine misbehavior**: taxonomy of misbehaviors that can lead to disagreement — equivocation and amnesia — and how each can be detected
- [specs/consensus/accountable-tm/README.md](specs/consensus/accountable-tm/README.md) — **Accountable Tendermint**: variant that enables the detection of amnesia attacks, in particular when agreement is violated
- [specs/consensus/accountable-tm/pseudo-code.md](specs/consensus/accountable-tm/pseudo-code.md) — **Accountable Tendermint pseudo-code**: changes in the original pseudo-code in order to implement this variant

### English specifications for block streaming

`specs/block-streaming/` contains the Quint spec for streaming proposal parts (used when a proposed value is split into ordered `INIT`/`DATA`/`FIN` messages and reassembled by the receiver):

- [specs/block-streaming/README.md](specs/block-streaming/README.md) — **Proposal parts streaming**: overview of the Quint model (`part_stream.qnt`, backed by `binary_heap.qnt` and `spells.qnt`) with `quint verify` invocations for the safety invariant and termination property, plus example counter-examples

### English specifications for synchronization

`specs/synchronization/` documents the protocols nodes use to catch up when they fall behind the tip of the chain:

- [specs/synchronization/README.md](specs/synchronization/README.md) — **Index**: entry point listing the synchronization protocols
- [specs/synchronization/valuesync/README.md](specs/synchronization/valuesync/README.md) — **ValueSync (MVP)**: full English specification of the ValueSync protocol (originally "Blocksync") — motivation, client/server/consensus composition, status/request/response message formats, synchronization strategy, the Quint formalization in `valuesync/quint/`, state-machine variants (with and without consensus), and known issues

### English specifications for the network layer

`specs/network/` covers the libp2p-based networking stack — how nodes find each other and how they disseminate messages:

- [specs/network/README.md](specs/network/README.md) — **Index**: overview of the network layer and pointers to the peer-discovery and gossip protocols
- [specs/network/discovery/README.md](specs/network/discovery/README.md) — **Peer discovery**: problem statement and model of the discovery protocol — bootstrap nodes, joining without knowing the full network, and assumptions about honest/Byzantine behavior
- [specs/network/discovery/ipd-protocol.md](specs/network/discovery/ipd-protocol.md) — **Iterative Peer Discovery (IPD)**: detailed description of the IPD algorithm — properties (discoverability, Byzantine-resilience, termination), assumptions, and protocol steps
- [specs/network/gossip/README.md](specs/network/gossip/README.md) — **Gossip protocol**: placeholder/stub for documentation of the `gossipsub`-based message dissemination layer (minimal content today)
