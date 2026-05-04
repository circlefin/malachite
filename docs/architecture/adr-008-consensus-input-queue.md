# ADR 008: Consensus Input Queue

## Changelog
* 2026-03-10: Initial draft

## Context

In BFT consensus, gossip messages (votes, proposals, proposed values) can arrive ahead of a node's
current height. When a node is at height `H`, it may receive messages for height `H+1` or higher
from faster peers. Without buffering, these messages would be dropped, forcing peers to retransmit
them—wasting bandwidth and delaying consensus.

Additionally, messages can arrive for the current height before the consensus driver has initialized
(i.e. while the round is still `Nil`). These messages must also be held until the driver is ready.

The consensus input queue addresses both of these problems by buffering future-height and
pre-initialization messages and replaying them when the node advances to the corresponding height.

## Decision

### Data Structure: `BoundedQueue<I, T>`

The input queue is implemented as a generic `BoundedQueue<I, T>` defined in
`core-consensus/src/util/bounded_queue.rs`:

```rust
pub struct BoundedQueue<I, T> {
    capacity: usize,
    per_key_capacity: usize,
    queue: BTreeMap<I, Vec<T>>,
}
```

- `I` is the index type (consensus height).
- `T` is the value type (`Input<Ctx>`).
- `capacity` bounds the number of unique indices (heights), not the total number of values.
- `per_key_capacity` bounds the number of values stored under a single index (height).
- `BTreeMap` maintains sorted order for efficient range operations.

The consensus state holds the queue as:

```rust
pub input_queue: BoundedQueue<Ctx::Height, Input<Ctx>>
```

### Eviction Policy

When the queue is full and a new height arrives:

| Condition | Action |
|---|---|
| Index already exists, per-key capacity not reached | Append to existing `Vec` — succeeds |
| Index already exists, per-key capacity reached | Reject — returns `false` |
| Queue not full | Insert new entry — succeeds |
| Queue full, new index < max index | Evict highest index, insert new — succeeds |
| Queue full, new index >= max index | Reject — returns `false` |

This policy prioritizes messages for nearer heights (which the node will reach sooner) over more
distant ones. The `BTreeMap::last_entry()` method provides O(log n) access to the highest index.

The `shift()` method removes all entries below a given index using `BTreeMap::split_off()` in
O(log n), which is used to prune stale entries when advancing to a new height.

### What Gets Buffered

Three of the nine `Input` variants are buffered:

| Handler | Condition | Buffered Input |
|---|---|---|
| `vote.rs` | `vote_height > consensus_height` | `Input::Vote(signed_vote)` |
| `vote.rs` | `round == Nil` (driver not started) | `Input::Vote(signed_vote)` |
| `proposal.rs` | `proposal_height > consensus_height` | `Input::Proposal(signed_proposal)` |
| `proposal.rs` | `round == Nil` | `Input::Proposal(signed_proposal)` |
| `proposed_value.rs` | `value_height > consensus_height` | `Input::ProposedValue(value, origin)` |

The remaining variants (`StartHeight`, `PolkaCertificate`, `RoundCertificate`, `Propose`,
`TimeoutElapsed`, `SyncValueResponse`) are processed immediately or handled through other paths.

### Message Flow

```
Gossip message arrives
        │
        ▼
  Handler (vote/proposal/proposed_value)
        │
        ├── height < consensus_height  →  DROP (stale)
        ├── height == consensus_height  →  PROCESS immediately
        ├── height > consensus_height   →  BUFFER in input_queue
        └── round == Nil (not started)  →  BUFFER in input_queue
                                                │
                                                ▼
                                     BoundedQueue.push(height, input)
                                                │
                    ┌───────────────────────────┘
                    ▼
          On StartHeight(H):
            1. reset_and_start_height()
            2. on_start_height()
            3. replay_pending_msgs()
                  │
                  ▼
            state.take_pending_inputs()
              → shift_and_take(&H)
              → removes entries < H
              → extracts entries == H
              → replays each via handle_input()
```

### Replay on Height Transition

When the node advances to a new height via `StartHeight`:

1. `reset_and_start_height()` resets consensus state and moves the driver to the new height.
2. `on_start_height()` starts round 0 and applies the `NewRound` driver input.
3. `replay_pending_msgs()` drains the queue:
   - Calls `state.take_pending_inputs()` which invokes `shift_and_take(&current_height)`.
   - This removes all entries with height < current height (cleanup) and extracts entries at
     the current height.
   - Each extracted input is replayed through `handle_input()`.

On restart (`is_restart == true`), buffered inputs are taken (to free memory) but **not** replayed,
because the WAL handles state recovery instead.

### Signature Verification

Buffered messages bypass signature and proposer verification at buffer time. Verification requires
`consensus_height == message_height`, which does not hold for future-height messages. Validation
happens during replay when the message is processed through `handle_input()`.

This means the queue can hold invalid messages from Byzantine peers, but they are filtered out at
replay time.

### Configuration

The queue is configurable via two `ConsensusConfig` fields:

| Setting | Default | Rationale |
|---|---|---|
| `queue_capacity` | 10 | Max unique future heights buffered |
| `queue_per_height_capacity` | 500 | Max messages per height (for `n` validators: `2n − 1` per round × expected rounds) |

Test applications use `queue_capacity = 100` for more headroom.

The sync module uses a separate `BoundedQueue` instance (`sync_queue`) with capacity derived from
`2 * parallel_requests * batch_size` (used for both `capacity` and `per_key_capacity`).

### Metrics

Two metrics are updated on every `buffer_input()` and `take_pending_inputs()` call (when the
`metrics` feature is enabled):

- `queue_heights`: number of unique heights currently buffered (`len()`)
- `queue_size`: total number of messages buffered across all heights (`size()`)

### StateDump

The `StateDump` struct (used for debugging) clones the entire `input_queue`, providing visibility
into what messages are buffered at any point.

## Status

Accepted

## Consequences

### Positive

- The input queue allows the consensus driver to process messages as soon as they arrive, without
  waiting for the node to reach the corresponding height. This reduces latency and bandwidth
  overhead from retransmissions.
- The eviction policy ensures that messages for nearer heights are prioritized, which is more likely to benefit the node's progress.

### Negative

- None

## References

- `code/crates/core-consensus/src/util/bounded_queue.rs` — `BoundedQueue` implementation
- `code/crates/core-consensus/src/state.rs` — `input_queue` field and `buffer_input()` / `take_pending_inputs()` methods
- `code/crates/core-consensus/src/handle/vote.rs` — vote buffering logic
- `code/crates/core-consensus/src/handle/proposal.rs` — proposal buffering logic
- `code/crates/core-consensus/src/handle/proposed_value.rs` — proposed value buffering logic
- `code/crates/core-consensus/src/handle/start_height.rs` — replay logic on height transition
- `code/crates/config/src/lib.rs` — `queue_capacity` configuration
- `code/crates/engine/src/sync.rs` — sync queue (separate `BoundedQueue` instance)
- `code/crates/engine/src/consensus/state_dump.rs` — `StateDump` inclusion of input queue
