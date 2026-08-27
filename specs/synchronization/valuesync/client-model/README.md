# ValueSync Client Quint model

An AI-generated formal [Quint](https://quint-lang.org) model of the Malachite **ValueSync**
client — the mechanism by which a node that has fallen behind catches up on already-decided values
(blocks + commit certificates) from its peers.

The model covers the **client** (syncing) side of the state machine against an abstract **peer
oracle**. It is an executable model: every property below is checked by sampled simulation
(`quint run` / `quint test`), not just typechecked.

## Files

| File | Contents |
|------|----------|
| `valuesync.qnt` | The model: types, pure logic, actions, invariants, witnesses, concrete instances, scope, and abstractions. |
| `valuesyncTest.qnt` | Deterministic `run` tests for catch-up, retries, and peer-status changes. Imports `valuesync`. |

### Correspondence to the code

The model grounds these files (update the model and re-check its invariants *before* changing them):

- `code/crates/sync/src/handle.rs` — client + server handlers
- `code/crates/sync/src/state.rs` — `State`, peer / range selection
- `code/crates/engine/src/sync.rs` — buffering by `consensus_height`, decide / started-height wiring

## Prerequisites

Quint **≥ 0.32.0** (`npm install -g @informalsystems/quint`). Check with `quint --version`.

Run all commands from this directory (`specs/synchronization/valuesync/client-model`).

## Typecheck

```bash
quint typecheck valuesync.qnt
quint typecheck valuesyncTest.qnt
```

No output means success.

## Run the deterministic tests

```bash
quint test valuesyncTest.qnt --main=valuesyncTest
```

## Check the invariants (sampled simulation)

The model exposes two concrete instances (modules) to run against:

| Instance | PEERS | BATCH_SIZE | MAX_PARALLEL | TOP |
|----------|-------|-----------|--------------|-----|
| `vs`     | 3     | 2         | 2            | 4   |
| `vsBig`  | 4     | 3         | 3            | 6   |

Check all invariants in one run:

```bash
quint run valuesync.qnt \
  --main=vs \
  --max-steps=30 \
  --max-samples=1000 \
  --invariants invSyncAboveTip invSyncUncovered invDisjoint invNoOrphanedOwner invNoLostHeight \
               invNoCapacityBlockedFrontier invBudgetRespected \
  --verbosity=1
```

Swap `--main=vs` for `--main=vsBig` to stress a larger configuration.

Expected results:

| Invariant | Meaning | Result |
|-----------|---------|--------|
| `invSyncAboveTip`   | `sync_height > tip` once started | holds |
| `invSyncUncovered`  | `sync_height` never inside a pending range | holds |
| `invDisjoint`       | pending ranges pairwise disjoint | holds |
| `invNoOrphanedOwner`| no pending entry owned by a peer absent from `known` | holds |
| `invNoLostHeight`   | every height in `(tip, sync_height)` is covered by a pending request | holds |
| `invNoCapacityBlockedFrontier` | when no request is outstanding and an eligible peer holds the gap, a frontier request is always possible | holds |
| `invBudgetRespected` | tracked requests awaiting a response never exceed `parallel_requests` | holds |

`invBudgetRespected` bounds the requests ValueSync tracks, not the requests live on the wire. The
engine keeps its own map of transport requests, and two paths drop a tracked entry without ending
its transport request: a height restart clears `pending_requests` while the engine clears only its
value queue, and pruning drops an entry once consensus passes its range. Each orphan ends on its
own response or timeout, so the excess is a window, not a leak. The model has no transport map and
omits the restart path, so it cannot see either.

If an invariant is ever violated (e.g. while iterating on the model), capture the counterexample
trace with `--mbt --out-itf`:

```bash
quint run valuesync.qnt --main=vs --max-steps=30 --max-samples=2000 \
  --invariant=<invariant> --mbt --out-itf=cex.itf.json
```

The failure output includes a seed. Copy it from the `Use --seed=...` line and replay the same
execution with additional detail:

```bash
quint run valuesync.qnt --main=vs --max-steps=30 \
  --invariant=<invariant> --seed=<seed> --backend=rust --verbosity=3
```

## Check reachability (witnesses)

Witnesses confirm the interesting states are reachable (a 0% witness means a dead action):

```bash
quint run valuesync.qnt --main=vs --max-steps=30 --max-samples=1000 \
  --witnesses wStarted wPending wParallel wPlaceholder wRetried wRetriedReservation \
              wPrunedStatus wProgress wCaughtUp wReroutedPastDisconnected
```

`wCaughtUp` (full catch-up to `TOP`) fires in roughly 45% of traces; the rest fire in a healthy
fraction too. `wParallel` and `wReroutedPastDisconnected` are the rarest (~0.5% and ~1%).

`wRetriedReservation` reads a latch that the retry actions set, not a shape of the state. Which
entry a retry started from is a property of the step, and every retry ends in an in-flight entry,
so no predicate over the resulting state can tell a reservation retry from an ordinary one.

`wCapacityBlockedFrontier` is the forbidden-state witness negated by
`invNoCapacityBlockedFrontier`. It should remain unreachable. When checking a suspected regression,
search for a counterexample trace without scripting the interleaving:

```bash
quint run valuesync.qnt --main=vs --max-steps=30 --max-samples=2000 \
  --witnesses wCapacityBlockedFrontier --mbt --out-itf=cex.itf.json
```

> Note: `quint run` (sampled) is the primary workflow. For exhaustive bounded checking with
> Apalache, see [Optional: exhaustive verification with Apalache](#optional-exhaustive-verification-with-apalache) below.

## Optional: exhaustive verification with Apalache

The model is Apalache-compatible (height scans use constant `0.to(MAXH)` ranges so Apalache can
encode them), so `quint verify` can bounded-model-check it exhaustively.

Prove an invariant holds up to a depth bound (full BFS — it must rule out every interleaving, so
this is the slow direction):

```bash
quint verify valuesync.qnt --main=vs --invariant=invNoLostHeight --max-steps=10
```

Use `--random-transitions` when hunting a suspected violation; omit it to explore with full BFS.

Apalache writes a `_apalache-out/` directory (gitignored).
