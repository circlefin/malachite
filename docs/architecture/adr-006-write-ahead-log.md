# ADR 006: Consensus Write-Ahead Log (WAL)

## Changelog

* 2026-01-27: Initial version

## Context

Malachite should adhere to the **crash-recovery** failure model.
This means that it should tolerate processes that rejoin the computation
-- i.e., _recover_ -- after a _crash_ or after being shut down.
And should do so in a **consistent** way, which can be generally defined as
follows: a recovering process should be indistinguishable, in terms of its
outputs, from a process that paused its computation for a long while.

In order to maintain correctness, i.e. to behave in a consistent way after a
crash, a process needs to:

1. Log all relevant _inputs_ processed during ordinary execution to persistent storage;
2. Upon recovery, retrieve from persistent storage and execute -- i.e., _replay_ -- the logged inputs.

A common technique to support crash-recovery is the Write-Ahead Log (WAL).
A WAL is an append-only registry (typically a file), to which inputs are logged
_before_ they are applied to the consensus state machine.
Upon recovery, a process sequentially reads inputs logged to the WAL and
replays them.
As a result, the state of a process after replaying the WAL should be identical
to its state before being shut down.

The adoption of a WAL as a crash-recovery technique presumes that
the consensus implementation is **deterministic**.
This means that given an initial state and a sequence of inputs,
the consensus state and the outputs produced when applying each input
will always be the same.
It this is not the case, the outputs produced by a recovering process may
differ from the ones produced before crashing, with an associated risk of
producing _equivocating_ outputs -- which renders the recovering process
a slashable Byzantine process.

## Implementation

This section discusses the Tendermint consensus implementation in Malachite
and how the consensus WAL can be implemented.

### Layers

The core of the BFT Tendermint consensus implementation in Malachite,
the [malachitebft-core-state-machine][smr-crate] crate,
is a deterministic state machine.
Its [inputs](./adr-001-architecture.md#input-events-(internal-apis)-1),
however, are aligned with the `upon` clauses of Tendermint's
[pseudo-code][pseudo-code] and represent the so-called _complex events_.
For instance, the reception of a single `Precommit` vote message does not
constitute an input for this state machine, while the reception of `Precommit`
messages for the same round from `2f + 1` voting-power equivalent processes
is an input (e.g., `PrecommitAny`) for it.
While only complex events do produce state transitions and outputs,
the definition of consistency above requires the logging of single inputs.

> For reviewers: the affirmation above is true, but can be contested.

The second layer of the consensus implementation,
the [malachitebft-core-driver][driver-crate] crate,
collects single inputs to produce complex events for the consensus state machine.
The [driver](./adr-001-architecture.md#consensus-driver) is also responsible for
removing the non-determinism present in Tendermint's [pseudo-code][pseudo-code],
where in the case when multiple `upon` clauses can be activated, any of them
could be chosen.
The driver establishes priorities between them so that to render the operation
deterministic.
So, ideally, it should be the right layer to invoke the WAL primitive for the
logging of inputs and for replaying WAL entries retrieved from persistent
storage upon recovery.

There are some issues, however, for implementing the WAL primitives at the
driver's layer.
The first is the fact that it adopts the Tendermint concept of `Proposal`, as a
self-contained message carrying the proposed value.
As discussed in [ADR 003](./adr-003-values-propagation.md), in Malachite the
dissemination and ordering of values are detached.
The dissemination is usually a role implemented by the application, that reports
to Malachite received values and their validity, via the `ProposedValue` event.
This event, typically but not necessarily combined with the reception of
consensus `Proposal` message, form the `Proposal` input provided to the driver.
Thus, the driver receives a complex event but does not keep record of the
single inputs, whose persistence is required by the adopted definition of
consistency.

> For reviewers: again, the affirmation above is true, but can be contested.

A second reason that prevents the driver from being the right layer to insert
the WAL logic is related to the signatures included in consensus messages.
Signature verification is not performed by the driver, that only receives a
message if its authenticity was certified. 
The problem is that messages logged to the WAL are supposed to be complete and
contain their signatures.
In particular because signatures are included in some outputs produced
Malachite, mainly the `Decision` output that includes signatures from a set of
`Precommit` messages.

The third layer of the consensus implementation,
the [malachitebft-core-consensus][consensus-crate] crate,
interacts with external resources to request some actions to be performed,
either synchronously or asynchronously.
This includes signing and verifying message signatures, via the `Sign*` and
`VerifySignature` effects.
And could also include the implementation of the WAL primitives: append or
replay an input.
Notice that, whichever layer employs the WAL append primitive, at the end is
the `core-consensus` layer that will interact with its implementation.

### Inputs

A priori, all inputs that change the consensus protocol state or produce an
output should be persisted to the WAL.
More specifically:

1. Consensus messages: `Proposal` and `Vote`, the last representing both
   `Prevote` and `Precomit` votes;
2. Expired timeouts `TimeoutElapsed(step)`, where `step` is one of
   `{propose,prevote, precommit}`;
3. Application input `LocallyProposedValue`: the return of `getValue()` helper
   at the proposer;
4. Application input `ProposedValue`: received consensus value `v` and its
   validity `valid(v)`;

The case of consensus messages is straightforward, as their reception leads to
progress in the consensus protocol.

The case of expired timeouts is less evident.
Timeouts are scheduled when some conditions on the received messages are
observed.
While their expiration leads to state transitions, provided that the process is
still in the same consensus step when they were scheduled.
When the process is replaying inputs from the WAL during recovery, the ordinary
consensus execution should schedule the same timeouts scheduled before the
process has crashed, while processing the same inputs.
But since it takes time for the timeouts to expire, it is hard to ensure that
the state of the process when the timeout expires will be the same it was
before it had crashed.
As the next state and outputs of the timeout expiration event depends on the
process state, it must be ensured that the `TimeoutElapsed` inputs are
replayed during recover in _the same relative order_ to other inputs they were
before crashing.
In other words, since _time_ is non deterministic, time-based event should be
logged.

The values proposed by the local instance of the application, when the
process is the proposer of a round, are also an important source of
non-determinism.
As typical applications produce consensus values from values received from
clients, it is unlikely that the return value of `getValue()` when a process is
recovering will be the same as it was before the process has crashed.
It is true that the application should be consistent and also support the
crash-recovery behavior, returning the same value upon multiple calls to
`getValue()`: this is actually a [requirement](TODO-link).
But since the return of a `getValue()` call produces a `Proposal` message that
is broadcast, it is safer to just store the value returned by the application,
which it is supposed to be small as large values are propagated by the
application (see [ADR 003](./adr-003-values-propagation.md)).

The `ProposedValue` inputs received from the application are typically combined
with the `Proposal` consensus message received by the process to produce the
`Proposal` input that is processed by Tendermint's state machine.
This operation is also discussed in [ADR 003](./adr-003-values-propagation.md).
In the same way as for the `LocallyProposedValue` input, the application is
supposed to be deterministic and consistent, replaying the same inputs when the
process recovers.
But since the reception of these inputs typically lead to state transitions and
outputs, it is safer to just store the value returned by the application, which
it is supposed to be small, together with its application-evaluated validity.

> For reviewers: while looked obvious in the first design, persisting the
> application inputs (values) appears not to be so crucial, when a
> well-behaving application is considered.

> **NOTE**: consensus inputs that were added afterwards, namely
> `SyncValueResponse`, `RoundCertificate`, `PolkaCertificate`, and
> `CommitCertificate` are not persisted.
> This may lead to inconsistent behavior, or not, which needs to be checked.

### Checkpoints

A WAL enables crash-recovery behavior in systems that can be modelled as
deterministic state machines.
This, for example, that starting from an initial state `s0` and applying
inputs `i1` and `i2`, the system transitions to states `s1` and `s2`, respectively.
This also means that starting from state `s1` and only applying input `i2`,
the state machine is also replayed until it reaches the same state `s2`.
State `s1` in the example is a checkpoint.
Notice that by starting from state `s1`, the outputs produced by the transition
`s0` to `s1` are not replayed.

Checkpoints for the Tendermint state machine can be safely produced at the
**beginning of each height** because, from the consensus point of view, heights
are completely independent from each other.
This means that if a process is at height `H`, no input pertaining to a height
`H' < H` will produce any state transition or output.
Thus, there is no need to replay inputs and revisiting states belonging to
previous heights.

In practical terms, this means that upon a `StartHeight` input for height `H`,
all logged entries referring to heights `H' < H` can be removed from the WAL.
Assuming that inputs for future heights `H" > H'` are not logged to the WAL,
when `StartHeight` is received for height `H`:

1. Height `H` was never started by the process, and all WAL entries are from
   height `H' < H`, typically `H' = H - 1`;
2. OR height `H` was previously started, the WAL contains inputs for height `H`
   that have to be replayed, since this is a recovery;

Case 1. is the ordinary case, with no crashes or restarts involved.
The process can just **reset** the WAL to height `H`.
Namely, to remove all inputs possibly present in the WAL,
that by design must refer to previous heights,
and set up the WAL to log inputs of height `H`.

Case 2. refers to when the process is restarting or recovering,
and has to replay the WAL content, as described in the [Replay](#replay) section.

### Persistence

The correct operation of a WAL-based system requires logging inputs to the WAL
**before** they are applied to the state machine.
A more precise requirement establishes the relation of persistence of inputs
and production of outputs: all inputs that have lead to the production
of an output, as a result of a state machine transition, must be persisted
before **the output is emitted** (to the "external word").
The reason is the adopted definition of consistency, which is derived from the
outputs produced by a process during regular execution and recovery.

Although seemingly complex, there is very simple definition of
"all inputs that have lead to the production of an output":
every processed input preceding the production of the output.
This definition enables the following operational design for the WAL component:

1. Log all processed inputs, in the order they have been processed, without
   persistence guarantees -> asynchronous writes;
2. When an output is produced, and before emitting it, persist all inputs that
   were not yet persisted -> synchronous writes or `flush()`.

Put in different words, inputs that do not (immediately) lead to an output, or
to a relevant state transition, can be logged in foreground and in a best
effort manner.
While the production of an output demands a synchronous, blocking call to
persist all the outstanding inputs.

> It is not 100% clear to me if we adopted this approach.
> I have the impression that all WAL append calls are synchronous.

### Replay

All discussion of previous boils down to this point: how is the operation of a
recovery process?
A first and relevant consideration is that Malachite, a priori, does not
know if it either in ordinary or recovery operation.
The consensus layer, in either operation mode, waits for a `StartHeight` input
from the application indicating the height number `H` to start.
At this point Malachite should open and load the WAL contents to check if it
includes entries (inputs) belonging to height `H`:
if there are, it is in recovery mode; otherwise, in regular operation.
If the concept of [Checkpoints](#checkpoints) is adopted, this verification is
even simpler and already described on the associated section (Case 2).

When the application requests Malachite to start height `H` via `StartHeight`
input, the consensus [driver](./adr-001-architecture.md#consensus-driver) is
configured with its initial state, set for height `H` with some parameters
applied (e.g., the validator set).
If there are WAL entries belonging to height `H`, the process is in recovery
mode: all height `H` inputs are replayed in the order with which they appear in
the WAL.
Once there are no (further) inputs to be replayed, the process starts its
ordinary operation, processing inputs from the network, from the application,
and from other protocols.

It remains to clarify what is the difference between replaying (during recovery)
and processing (in regular operation) an input?
In theoretical terms, none.
The replayed inputs are inputs that were received and applied by the process
before crashing, producing state transitions and outputs.
Upon recovery, the same inputs are read from the WAL and applied, producing the
same state transitions and outputs.
Consensus protocols in general, and Tendermint in particular, are able to handle
duplicated inputs, so there is not actual harm to correctness.

There is also an important corner case to be considered.
Crashes can occur at any time, so in particular they can occur when an output
was produced but not yet emitted.
Assume that WAL replay is implemented in a way that outputs derived from
replayed inputs are produced but not emitted.
So there is a case where the process transitions to a particular state (say,
the `precommit` step of a round) but no process sees the output produced by
that state transition (in the case, a `Precommit` message for that round)
because it is not emitted during recover.

In practical terms, however, the question is whether is acceptable, during recovery,
to emit the same outputs "again"?
Or which outputs and associated [Effects](./adr-004-coroutine-effect-system.md#effect)
should be produced and handled during recovery?
Notice that, in particular, logging an input to the WAL is an `Effect`, but in
this case does it make sense to append to WAL inputs that were originally
replayed from that same WAL?

> I am not sure on how this is handled in the current implementation.
> From the `wal_replay` method from `engine/src/consensus.rs`, replayed inputs
> are processed using the `process_input` method, the same used for ordinary
> inputs.
>
> At the same time, there is a `Recovering` phase set during replay, that
> should change some behaviors.
> From what I could check, it only prevents logging replayed inputs to the WAL.

## Decision

> This section explains all of the details of the proposed solution, including implementation details.
It should also describe affects / corollary items that may need to be changed as a part of this.
If the proposed change will be large, please also indicate a way to do the change to maximize ease of review.
(e.g. the optimal split of things to do between separate PR's)

## Status

Accepted

## Consequences

> This section describes the consequences, after applying the decision. All consequences should be summarized here, not just the "positive" ones.

### Positive

### Negative

### Neutral

## References

> Are there any relevant PR comments, issues that led up to this, or articles referenced for why we made the given design choice? If so link them here!

* {reference link}

[smr-crate]: https://github.com/circlefin/malachite/tree/main/code/crates/core-state-machine
[driver-crate]: https://github.com/circlefin/malachite/tree/main/code/crates/core-driver
[consensus-crate]: https://github.com/circlefin/malachite/tree/main/code/crates/core-consensus
[pseudo-code]: https://github.com/circlefin/malachite/blob/main/specs/consensus/pseudo-code.md
