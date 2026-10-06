# ADR 010: Sync Inbound Resource Limits

## Changelog
* 2026-05-07: Initial draft
* 2026-09-18: Inbound `batch_size` now clamps an over-long request instead of rejecting it

## Context

The sync request-response protocol had no per-peer rate limit, no inbound
concurrency cap, and no host-call timeout on `Effect::GetDecidedValues`. Any
libp2p peer could drive the node to fetch decided values from the host actor
at will, with the spawned tokio task waiting indefinitely on a oneshot channel
because `call_and_forward(..., None)` disables ractor's timeout.

The threat model distinguishes two attack shapes:

1. **Single-peer flooding.** One peer sends many requests from one connection.
2. **Fair-share monopolization.** One peer holds a disproportionate fraction of
   the shared server-side capacity, starving honest peers even while staying
   within per-peer request-rate budgets.

The trust perimeter assumes validators are non-malicious, so the immediate
operational risk is low. The defenses below still apply because (a) the same
protocol may be used outside the trusted perimeter, and (b) discovery exposes
the node to unauthenticated libp2p peers.

## Decision

Four layered defenses at the sync protocol receiver, composed so each attack
shape is caught by the earliest cheap check.

### Per-peer rate limit (`governor`)

A `governor`-backed GCRA limiter keyed by `PeerId` is checked first in
`on_value_request`.

```rust
pub type InboundRateLimiter =
    RateLimiter<PeerId, DefaultKeyedStateStore<PeerId>, DefaultClock>;
```

The quota is derived from two config fields: `burst = max_inbound_requests_per_window`,
`replenish_interval = inbound_request_rate_limit_window / burst`. Stale per-peer
entries are pruned by a dedicated background ticker via `retain_recent()`
firing on `Msg::PruneInboundRateLimiter`. The ticker fires every
`inbound_request_rate_limit_window` regardless of `status_update_interval` —
pruning has to run even in `Eager` status-update mode (where the
status-update ticker is absent and `Msg::Tick` never fires).

### Per-peer in-flight cap

A counter per `PeerId` in `sync::State`, plus a reverse map from admitted
request to originating peer for decrement lookup.

```rust
pub inbound_peer_inflight: HashMap<PeerId, u32>,
pub inbound_request_peer: HashMap<InboundRequestId, PeerId>,
```

Cap = `parallel_requests` (default 5). Checked after the rate limit; if the
peer is already at cap, `on_value_request` emits an empty response. On success,
both maps are updated immediately before emitting `Effect::GetDecidedValues`.

Decrement is per-request, driven by `Input::InboundRequestEvicted` and by
`on_got_decided_values`:

- **Host reply** — `on_got_decided_values` decrements the counter and forwards
  the values (or an empty response if the host had nothing) via
  `Effect::SendValueResponse`.
- **Host-call timeout** — the engine's `Timeout::InboundRequest` handler emits
  `Input::InboundRequestEvicted`, which decrements the counter without any
  further response.
- **Admission-semaphore rejection** — the engine self-casts
  `Msg::GotDecidedValues(_, _, vec![])` so the sync handle both releases the
  slot and sends an empty response.
- **Late reply for an already-evicted request** — the engine drops the values
  and emits `Input::InboundRequestEvicted` so the counter is still decremented.

Peer disconnect deliberately does **not** clear the counter or reverse map:
spawned host-call tasks from before the disconnect still hold their admission
permits until they end. Clearing on disconnect would let a reconnecting peer
take fresh admission slots on top of the still-running ones, breaking the
per-peer fair-share cap under disconnect/reconnect churn. The counter drains
naturally as tasks complete or as `Timeout::InboundRequest` fires.

Like `batch_size`, this limit applies in both directions: outbound (what we
send) and inbound (what we accept). The two differ in how they treat a request
over the limit — `parallel_requests` rejects it, while `clamp_request_range`
shortens it to our own `batch_size` at `sync/src/handle.rs`.

### Admission + execution semaphores

Two tokio `Arc<Semaphore>`s on the engine-layer sync actor:

```rust
pub struct State<Ctx: Context> {
    // ...
    inbound_admission_permits: Arc<Semaphore>,  // max_concurrent + max_pending
    inbound_execution_permits: Arc<Semaphore>,  // max_concurrent
}
```

- **Admission** (default 96) is acquired with `try_acquire_owned` in the
  `Effect::GetDecidedValues` handler. Rejection past this cap is synchronous
  and sends an empty response directly on the network.
- **Execution** (default 32) is acquired with `acquire_owned().await` inside
  the spawned tokio task. Requests that hold an admission permit but cannot
  immediately execute wait in tokio's internal semaphore queue, bounded above
  by the admission cap.

Both permits are released when the spawned task ends, either on host reply or
on ractor timeout.

The engine also holds each task's `AbortHandle` on the `InboundRequest` entry
in `state.inbound`. Every eviction path — `Timeout::InboundRequest` and peer
disconnect — calls `abort_handle.abort()` before releasing the per-peer slot,
so the task's `.await` points return early and both permit guards drop
immediately. Without this the semaphores would stay saturated for up to
`request_timeout` past the eviction under churn (frequent disconnects, or a
consistently slow host that keeps tripping the stall timer).

### Host-call timeout

The ractor call receives `Some(request_timeout)`:

```rust
host.call(|reply_to| HostMsg::GetDecidedValues { range, reply_to }, Some(timeout)).await
```

ractor 0.15 drops the mapper closure (and thus its captured semaphore permits)
on timeout, guaranteeing release even if the host hangs.

### Request lifecycle

```
Inbound ValueRequest
        │
        ▼
  on_value_request (sync handle)
        │
        ├── rate limiter check ─── fail ──▶ empty SendValueResponse
        ├── per-peer in-flight check ──── fail ──▶ empty SendValueResponse
        ├── range validation ─────── fail ──▶ empty SendValueResponse
        ├── clamp range to tip height and batch_size
        └── increment inbound_peer_inflight, insert inbound_request_peer
                │
                ▼
        Effect::GetDecidedValues (engine effect handler)
                │
                ├── try_acquire_owned(admission) ─── fail ──▶ empty OutgoingResponse
                │                                            (self-casts GotDecidedValues(empty))
                └── tokio::spawn(task)
                        │
                        ▼
                  acquire_owned(execution).await
                        │
                        ▼
                  host.call(GetDecidedValues, Some(request_timeout))
                        │                        │
                        ├── Success(values) ─────┤
                        ├── Timeout ─────────────┤
                        └── SenderError ─────────┤
                                                 ▼
                  cast Msg::GotDecidedValues(request_id, range, values)
                        │
                        ▼
        on_got_decided_values (sync handle)
                │
                ├── decrement inbound_peer_inflight, remove inbound_request_peer
                └── Effect::SendValueResponse(values)
```

### Fair-share calculation

With `parallel_requests = 5` and `max_concurrent + max_pending = 96`,
a single peer can occupy at most 5/96 ≈ 5 % of the admission pool. An
attacker fully saturating their rate budget (1000 burst, 100 req/s) is
bottlenecked at 5 in-flight; the remaining 91 of any burst are rejected
immediately with empty responses. Honest peers retain ≥ 95 % of admission
capacity.

Contrast with the previous design (rate limit only, no per-peer in-flight cap):
an attacker could burst 96 requests, all within rate budget, monopolize all 96
admission slots, and cause honest peers to see immediate empty responses for
~50 ms per burst, repeating ~1 s later as rate tokens replenished — sustained
~10–15 % time-averaged denial of service.

### Configuration

All settings live on `ValueSyncConfig` and are plumbed through `spawn.rs` into
`sync::Config`:

| Setting                                  | Default | Purpose                                                                             |
|------------------------------------------|---------|-------------------------------------------------------------------------------------|
| `inbound_request_rate_limit_window`      | `10s`   | GCRA window over which burst tokens replenish                                       |
| `max_inbound_requests_per_window`        | 1000    | Governor burst capacity per peer (sustained rate = burst / window)                  |
| `parallel_requests`                      | 5       | Per-peer in-flight cap (reused; already controls outbound parallelism to each peer) |
| `max_concurrent_inbound_requests`        | 32      | Execution-semaphore capacity — concurrent host calls                                |
| `max_pending_inbound_requests`           | 64      | Admission-semaphore extra capacity beyond `max_concurrent` (total = 32 + 64 = 96)   |
| `request_timeout`                        | `10s`   | Ractor timeout on the host call (reused; already controls outbound timeouts)        |

## Status

Accepted

## Consequences

### Positive

- Single-peer DoS is bounded to 5/96 ≈ 5 % of admission capacity regardless of
  burst shape.
- Reuses existing `parallel_requests` setting — applied in both directions,
  like the existing `batch_size` enforcement; no new configuration surface for
  the in-flight cap.
- Each defense layer uses a standard primitive (`governor` keyed limiter,
  tokio `Semaphore`), with the counter + reverse-map being the only bespoke
  state added.
- Invariants are local: `on_value_request` and `on_got_decided_values` are
  the only mutators; the counter tracks admitted requests and is naturally
  bounded by the global admission cap.

### Negative

- `parallel_requests` becomes a network-wide parameter. Peers that run with a
  higher outbound `parallel_requests` than ours will see their tail requests
  rejected, so raising it ahead of the fleet requires coordination. `batch_size`
  is milder: an over-long request is served short rather than rejected, so a
  peer that raises it does not stall. Coordination is still needed, but of a
  different kind — the serving nodes only have to run a version that clamps,
  not raise their own `batch_size`, and the requester gains no throughput until
  they do raise it.
- Per-peer state (`HashMap<PeerId, u32>` + `HashMap<InboundRequestId, PeerId>`)
  grows with the number of unique peers that have recent in-flight requests;
  bounded by the admission cap, and entries for peers with 0 in-flight are
  removed immediately.
- `parallel_requests` now serves two symmetric roles. Changing its meaning in
  the future requires splitting it into separate outbound and inbound settings.

### Neutral

- `governor`'s GCRA differs subtly from a fixed-window limiter: the per-peer
  budget replenishes continuously rather than resetting at window boundaries.
  Worst-case burst tolerance is equivalent; typical burst shapes behave
  slightly differently.
- The admission/execution split allows brief bursts to be absorbed without
  immediate rejection, unlike a single-semaphore design.

## References

- `code/crates/sync/src/state.rs` — `InboundRateLimiter` type alias,
  `inbound_peer_inflight`, `inbound_request_peer` fields, construction of
  governor quota in `State::new`
- `code/crates/sync/src/handle.rs` — `on_value_request` (rate limit + per-peer
  check + admission), `on_got_decided_values` (release via
  `release_inbound_peer_slot`)
- `code/crates/sync/src/config.rs` — runtime `Config` with the four inbound
  settings
- `code/crates/engine/src/sync.rs` — `inbound_admission_permits`,
  `inbound_execution_permits` on the actor state, `Effect::GetDecidedValues`
  handler (`try_acquire_owned` admission + `tokio::spawn` task acquiring
  execution + host call with `Some(request_timeout)`), dedicated rate-limiter
  pruning ticker firing `Msg::PruneInboundRateLimiter`
- `code/crates/config/src/lib.rs` — `ValueSyncConfig` serde form of the settings
- `code/crates/app/src/spawn.rs` — translation from `ValueSyncConfig` to
  `sync::Config`
