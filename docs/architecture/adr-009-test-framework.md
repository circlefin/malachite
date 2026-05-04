# ADR 009: Integration Test Framework

## Changelog

* 2026-03-13: Initial version

## Status

Accepted

## Context

Testing a BFT consensus system requires validating complex multi-node interactions that go far beyond unit-level checks. Consensus protocols must satisfy both safety and liveness properties under adversarial conditions, including node crashes, network partitions, equivocation, and varying validator configurations. These scenarios involve intricate timing dependencies, state recovery, and event sequencing that cannot be adequately captured by testing individual components in isolation.

The integration tests for Malachite BFT need to:

- Orchestrate multiple consensus nodes running concurrently within a single test
- Control node lifecycle events (spawn, crash, restart, database reset) at precise points
- Observe and assert on consensus protocol events (proposals, votes, decisions, finalization)
- Simulate adversarial behavior through pluggable network middleware
- Verify properties like WAL replay correctness, vote rebroadcasting, and certificate propagation
- Support both validator nodes and full nodes (non-voting observers)
- Remain readable and maintainable as the number of test scenarios grows

Without a dedicated framework, each integration test would need to duplicate significant boilerplate for node setup, event monitoring, lifecycle management, and assertion logic.

The `arc-malachitebft-test-framework` crate provides a purpose-built testing harness that addresses these requirements through a fluent builder API and a step-based execution model.

## Decision

### Architecture Overview

The framework follows a builder-to-execution pipeline:

```
TestBuilder  →  Test  →  run_test()  →  NodeRunner  →  TestNode(s)
```

Tests are constructed declaratively using a builder pattern, then executed asynchronously with per-node concurrent task management via Tokio.

### Core Components

#### TestBuilder and Test

`TestBuilder<Ctx, S>` is the entry point for constructing a test scenario. It accumulates `TestNode` definitions and produces a `Test` instance. The type parameters are:

- `Ctx: Context` — the consensus context type (defines heights, addresses, validator sets, etc.)
- `S: Send + Sync + 'static` — optional per-node user state for tracking information across event handlers

`Test` holds the collection of nodes and provides `run()` and `run_with_params()` methods that execute the scenario with a timeout.

#### TestNode and the Step Model

Each `TestNode` represents a single participant in the test. Nodes are configured through a fluent builder API and internally accumulate a sequence of `Step` values that define their lifecycle:

```rust
pub enum Step<Ctx, S> {
    Crash(Duration),       // Crash the node after a delay
    ResetDb,               // Reset the node's database
    Restart(Duration),     // Restart the node after a delay
    WaitUntil(u64),        // Wait until a specific height is reached
    WaitUntilRound(u32),   // Wait until a specific round is reached
    OnEvent(EventHandler), // React to consensus events with a custom handler
    Expect(Expected),      // Verify the number of decisions made
    Success,               // Mark the test as successful
    Fail(String),          // Fail the test with a reason
}
```

Steps execute sequentially per node, while nodes themselves run concurrently. This model makes test scenarios read like a script describing each node's behavior over time:

```rust
let mut test = TestBuilder::new();

// Node 0: simple validator that runs to height 5
test.add_node()
    .start()
    .wait_until(5)
    .success();

// Node 1: crashes at height 2, restarts, and verifies it catches up
test.add_node()
    .start()
    .wait_until(2)
    .crash()
    .restart_after(Duration::from_secs(1))
    .wait_until(5)
    .expect_decisions(Expected::AtLeast(5))
    .success();

test.build()
    .run(Duration::from_secs(30))
    .await;
```

#### Event Handlers

Event handlers allow tests to observe and react to consensus protocol events. The framework provides both generic and specialized handler methods:

- **Generic:** `on_event(f)` — receives all `Event<Ctx>` values
- **Specialized:** `on_decided(f)`, `on_finalized(f)`, `on_proposed_value(f)`, `on_vote(f)` — type-safe handlers for specific event types

Handlers return a `HandlerResult` to control test flow:

- `WaitForNextEvent` — continue listening for events
- `ContinueTest` — move to the next step in the node's sequence

This enables assertions that depend on runtime state, such as verifying the structure of commit certificates or tracking signature counts across decisions.

#### Event Expectations

For common verification patterns, the framework provides declarative expectation methods that avoid the need for manual event handler boilerplate:

- `expect_wal_replay(at_height)` — verify WAL replay occurs at the given height
- `expect_vote_rebroadcast(at_height, at_round, vote_type)` — verify vote rebroadcasting
- `expect_round_certificate_rebroadcast(at_height, at_round)` — verify round certificate rebroadcasting
- `expect_skip_round_certificate(at_height, at_round)` — verify skip round certificate reception
- `expect_polka_certificate(at_height, at_round)` — verify polka certificate reception

#### NodeRunner Trait

The `NodeRunner<Ctx>` trait abstracts over the actual node implementation, allowing the framework to spawn, kill, and reset nodes without depending on a specific node implementation:

```rust
pub trait NodeRunner<Ctx: Context>: Clone + Send + Sync + 'static {
    type NodeHandle: NodeHandle<Ctx>;

    fn new<S>(id: usize, nodes: &[TestNode<Ctx, S>], params: TestParams) -> Self;
    async fn spawn(&self, id: NodeId) -> eyre::Result<Self::NodeHandle>;
    async fn reset_db(&self, id: NodeId) -> eyre::Result<()>;
}
```

The `HasTestRunner<R>` marker trait connects a `Context` to its `NodeRunner`, working around Rust orphan rules:

```rust
pub trait HasTestRunner<R>: Context {
    type Runner: NodeRunner<Self>;
}
```

#### Middleware

Nodes accept a pluggable `Middleware` implementation (via `with_middleware()`) that can intercept and modify network behavior. This is used to simulate adversarial scenarios such as:

- `PrevoteNil` — always vote nil (simulating a Byzantine validator)
- `PrevoteRandom` — vote for random values (simulating equivocation)

Middleware composes with the rest of the node configuration through `Arc<dyn Middleware>`.

#### TestParams

`TestParams` centralizes test-level configuration that applies to all nodes:

- Consensus settings: protocol type, RPC max size, consensus enabled/disabled
- Value sync: enabled, parallel requests, batch size, status update interval
- Block configuration: block size, transaction size, transactions per part, max retain blocks
- Vote extensions: optional extension configuration
- Discovery: enable/disable, persistent peer exclusions
- Timing: target block time, stable block times

The `apply_to_config()` method translates these parameters into the actual node `Config`.

#### Config Modifiers

Per-node configuration customization is supported through composable config modifiers:

```rust
node.add_config_modifier(|config| {
    config.consensus.p2p.protocol = Protocol::Quic;
});
```

Multiple modifiers compose via `Arc`, each receiving the already-modified config, enabling layered customization.

### Test Execution

When `Test::run()` is called:

1. **Logging initialization** — structured tracing is set up with per-crate log level filtering, respecting `MALACHITE_DEBUG` and `ACTIONS_RUNNER_DEBUG` environment variables for CI compatibility.

2. **Concurrent node execution** — each node runs as a Tokio task within a `JoinSet`, wrapped in a tracing error span for per-node context.

3. **Per-node lifecycle:**
   - An initial delay is applied if configured (`start_delay`)
   - The node is spawned via the `NodeRunner`
   - A background event monitoring task is launched to track decisions and height progression
   - Steps execute sequentially, with each step potentially killing, restarting, or waiting on the node

4. **Event monitoring** — a background task per node consumes the event stream and:
   - Increments atomic decision and height counters
   - Validates invariants (e.g., full nodes must not publish or receive consensus messages)
   - Captures WAL replay errors

5. **Completion** — all node tasks are joined; any failure or timeout causes the test to exit with a non-zero status.

### Full Node Support

Nodes can be designated as full nodes (non-voting observers) via `full_node()` or by setting `voting_power` to 0. The event monitoring task enforces that full nodes do not participate in consensus message exchange, catching integration-level regressions.

## Consequences

### Positive

1. **Readable test scenarios.** The fluent builder API and step-based model make integration tests read as sequential narratives, even though nodes run concurrently. New test scenarios can be written with minimal boilerplate.

2. **Comprehensive lifecycle testing.** The framework natively supports crash, restart, and database reset scenarios, which are essential for verifying WAL replay, state recovery, and liveness under failure.

3. **Extensible event verification.** Both generic event handlers and specialized expectation methods allow tests to verify fine-grained protocol behavior (certificate structure, vote extensions, rebroadcasting) without reimplementing monitoring logic.

4. **Pluggable adversarial simulation.** The middleware abstraction enables Byzantine behavior simulation at the network layer without modifying the consensus implementation.

5. **CI-friendly.** Built-in support for log level overrides via environment variables, deterministic port assignment via `NEXTEST_TEST_GLOBAL_SLOT`, and timeout enforcement ensure reliable execution in automated environments.

6. **Separation of concerns.** The `NodeRunner` trait decouples the test orchestration logic from the actual node implementation, allowing the framework to be reused with different node backends.

### Negative

1. **Learning curve.** The step-based execution model and the interaction between event handlers and step progression require familiarity that may not be immediately obvious to new contributors.

2. **Sequential step limitation.** Steps within a node execute sequentially, which can make it cumbersome to express scenarios where a node needs to react to multiple concurrent conditions.

3. **Implicit ordering.** The order of builder method calls matters (e.g., `start()` must come before `wait_until()`), but this is enforced at runtime rather than at compile time.

### Neutral

1. **Tight coupling to Tokio.** The framework depends on the Tokio async runtime for task management and synchronization. This is consistent with the rest of the codebase but limits use in non-Tokio environments.

2. **Atomic coordination.** Decision and height counters use `Arc<AtomicUsize>` for lock-free coordination between the event monitoring task and the step execution loop. This is efficient but requires careful reasoning about ordering guarantees.

## References

* [ADR 004: Coroutine-Based Effect System for Consensus](./adr-004-coroutine-effect-system.md) — the effect system that the test framework exercises
* [ADR 007: Write-Ahead Log](./adr-007-write-ahead-log.md) — WAL replay correctness, which the framework verifies via `expect_wal_replay()`
* [ADR 005: Value Sync](./adr-005-value-sync.md) — value synchronization protocol, tested through `TestParams` configuration
* `code/crates/test/framework/` — the framework implementation
* `code/crates/test/tests/it/` — integration tests that use the framework
