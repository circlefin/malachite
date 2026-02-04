# ADR 007: Consensus Write-Ahead Log (WAL)

## Changelog

* 2026-01-27: Initial version
* 2026-02-02: Version submitted for revision
* 2026-02-04: Reviewed and published version

## Context

Malachite should adhere to the **crash-recovery** failure model.
This means that it should tolerate processes that rejoin the computation
-- i.e., _recover_ -- after a _crash_ or after being shut down.
And should do so in a **consistent** way, which can be generally defined as
follows: a recovering process should be indistinguishable, in terms of its
outputs, from a process that paused its computation for a long while.

> Notice that consistency is not a "nice to have" property, but a required one.
> A process that is not consistent, does operate arbitrarily and becomes a
> potential Byzantine process.
> Recall that arbitrary behavior does not necessarily mean malicious behaviour:
> it can the result of bugs or misconfiguration.
>
> A textbook example of the impact of not properly handling the recovery of a
> process is the locking mechanism present in multiple consensus algorithms.
> In Tendermint, a process that emits a `Precommit` for a value `v` also
> _locks_ `v` at that round.
> The lock is a promise to not accept a value different than `v` in future
> rounds, except if a higher-round lock is produced.
> So, if a value `v' != v` is proposed in the next round `r`, the process
> must not issue a `Prevote` for `id(v')`: it must reject `v'` it by issuing a
> `Prevote` for `nil`.
>
> The lock on `v` is part of the state of the process.
> If the process is restarted but its consensus state is not properly
> recovered, then the process can, considering the example above, receive a
> proposal for `v' != v` in round `r` and misbehave in the following ways:
>
> * Amnesia: by "forgetting" about the promise associated to the pre-crash
>   lock on `v`, accept the proposed value `v' != v`;
> * Equivocation: emit a `Prevote` for `id(v')` in round `r`, while before
>   crashing it has emitted a `Prevote` for `nil` in round `r`.
>
> Note that this is just an example.
> There are multiple ways in which a process can misbehave if it loses its
> state upon recovery.
> For instance, in the same example, since `v` became a locked value, the
> proposer of round `r` must have re-proposed the valid value `v`.

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
If this is not the case, the outputs produced by a recovering process may
differ from the ones produced before crashing, with an associated risk of
producing _equivocating_ outputs -- which renders the recovering process
a slashable Byzantine process.

## Implementation

This section discusses the Tendermint consensus implementation in Malachite
and how the consensus WAL can be implemented.

### Layers

The BFT Tendermint consensus implementation in Malachite consists of multiple
layers, each of which could host the WAL implementation.

The core of Tendermint's implementation in Malachite is a deterministic state
machine: the [malachitebft-core-state-machine][smr-crate] crate.
The state-machine [inputs](./adr-001-architecture.md#input-events-(internal-apis)-1)
are aligned with the `upon` clauses of Tendermint's
[pseudo-code][pseudo-code] and represent the so-called **complex events**.
For instance, the reception of a single `Precommit` vote message does not
constitute an input for this state machine, while the reception of `Precommit`
messages for the same round from `2f + 1` voting-power equivalent processes
is an input (e.g., `PrecommitAny`) for it.

The second layer of the consensus implementation is the
[driver](./adr-001-architecture.md#consensus-driver) that collects
single inputs to produce complex events for the consensus state machine.
Implemented in the [malachitebft-core-driver][driver-crate] crate,
the driver is also responsible for removing the non-determinism present in
Tendermint's [pseudo-code][pseudo-code], which in situations when multiple
`upon` clauses can be activated, assumes that any of them could be chosen at
random.
The driver establishes priorities among potentially complex events, thereby making Malachite’s operation deterministic.

The third layer of the consensus implementation,
the [malachitebft-core-consensus][consensus-crate] crate,
is the interface between consensus and the host.
On the one hand, it receives inputs from the network, the application, or other
components, process and forward them to the driver.
On the other hand, it interacts with the host to request some actions
([Effects](./adr-004-coroutine-effect-system.md#effect) in Malachite's parlance)
to be performed.
For instance, core consensus layer receives messages from the network
(`Proposal` or `Vote` inputs), verifies their signatures (`VerifySignature`
effect), and forwards them to the driver.
If this processing results in a message, core consensus layer requests
its signing from the signer (`SignProposal` or `SignVote` effects) and its
broadcast to the network (`PublishConsensusMsg` effect) .
Since the WAL is a functionality implemented by the host, the interaction of
the consensus implementation with the WAL will happen through effects, produced
by core consensus layer.

The core consensus layer also intermediates the value
dissemination feature of Malachite.
As discussed in [ADR 003][adr-003], in Malachite the
dissemination and ordering of values are detached.
The dissemination is usually a role implemented by the application, that
reports to Malachite received values and their validity via the
`ProposedValue` core consensus input.
This input is then typically combined with `Proposal` core consensus input,
representing the reception of the corresponding message from the network,
to form the `Proposal` input provided to the driver and the state machine,
which are unaware of the operation of the value dissemination protocol.

The fourth and last layer of the consensus implementation is the 
[malachitebft-engine][engine-crate] crate, which provides a standard
implementation for all the effects, i.e., interactions with the host system,
requested by the core consensus layer.
It is the engine that actually implements or interacts with the network layer,
and it would be the engine to provide the interaction with the file system
needed to implement the consensus WAL.

### Inputs

A priori, all inputs that change the consensus protocol state or produce an
output should be persisted to the WAL.
More specifically:

1. Consensus messages: `Proposal` and `Vote`, the last representing both
   `Prevote` and `Precomit` votes;
2. Expired timeouts `TimeoutElapsed(step)`, where `step` is one of
   `{propose, prevote, precommit}`;
3. Application input `LocallyProposedValue`: the return of `getValue()` helper
   at the proposer;
4. Application input `ProposedValue`: received consensus value `v` and its
   validity `valid(v)`;

The case of consensus messages is straightforward, as their reception leads to
progress in the consensus protocol.

The case of expired timeouts is less evident.
Timeouts are scheduled when some conditions on the received messages are
observed.
Their expiration leads to state transitions, provided that the process is
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

The values proposed by the local instance of the application when the process
is the proposer of a round, via the `LocallyProposedValue` input, are also an
important source of non-determinism.
As typical applications produce consensus values from values received from
clients, it is unlikely that the return value of `getValue()` when a process is
recovering will be the same as it was before the process has crashed.
It is true that the application should be consistent and also support the
crash-recovery behavior, returning the same value upon multiple calls to
`getValue()`: this is actually a [requirement][issue-values].
But since the return of a `getValue()` call produces a `Proposal` message that
is broadcast, it is safer to just store the value returned by the application,
which it is supposed to be small as large values are propagated by the
application (see [ADR 003][adr-003]).

The `ProposedValue` inputs received from the application are typically combined
with the `Proposal` consensus message received by the process to produce the
`Proposal` input that is processed by Tendermint's state machine.
This operation is also discussed in [ADR 003][adr-003].
In the same way as for the `LocallyProposedValue` input, the application is
supposed to be deterministic and consistent, replaying the same inputs when the
process recovers.
But since the reception of these inputs typically lead to state transitions and
outputs, it is safer to just store the value returned by the application, which
is supposed to be small, together with its application-evaluated validity.

> Notice that, while apparently obvious in the first design, persisting the
> application inputs carrying proposed values and their validity is not really
> required if Malachite assumes a well-behaving (i.e., deterministic)
> application that replies consistently to Malachite requests.

### Checkpoints

A WAL enables crash-recovery behavior in systems that can be modelled as
deterministic state machines.
For example, when starting from an initial state `s0` and applying inputs `i1`
and `i2`, the system transitions first to state `s1`, then to state `s2`.
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

Case 2. refers to when the process is recovering and has to replay the WAL
content, as described in the [Replay](#replay) section.

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
"all inputs that have led to the production of an output":
every processed input preceding the production of the output.
This definition enables the following operational design for the WAL component:

1. Log all processed inputs, in the order they have been processed, without
   persistence guarantees -> asynchronous writes;
2. When an output is produced, and before emitting it, persist all inputs that
   were not yet persisted -> synchronous writes or `flush()`.

Put in different words, inputs that do not (immediately) lead to an output, or
to a relevant state transition, can be logged in background and in a best
effort manner.
While the production of an output demands a synchronous, blocking call to
persist all the outstanding inputs.

### Replay

All previous discussion boils down to this point: how is the operation of a
recovery process?
A key initial consideration is that, a priori, Malachite does not know whether it is operating in ordinary or recovery mode.
The consensus layer, in either operation mode, waits for a `StartHeight` input
from the application indicating the height number `H` to start.
At this point Malachite should open and load the WAL contents to check if it
includes entries (inputs) belonging to height `H`:
if there are, it is in recovery mode; otherwise, in regular operation.
If the concept of [Checkpoints](#checkpoints) is adopted, this verification is
even simpler and already described in the associated section (Case 2).

When the application requests Malachite to start height `H` via `StartHeight`
input, the consensus [driver](./adr-001-architecture.md#consensus-driver) is
configured with its initial state, set for height `H` with some parameters
applied (e.g., the validator set).
If there are WAL entries belonging to height `H`, the process is in recovery
mode: all height `H` inputs are replayed in the order with which they appear in
the WAL.
Once there are no (further) inputs to be replayed, the process starts its
ordinary operation, processing received inputs, starting from the ones buffered
while in recovery mode.

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
because it is not emitted during recovery.

In practical terms, however, the question is whether it is acceptable, during recovery,
to emit the same outputs "again"?
Or which outputs and associated [Effects](./adr-004-coroutine-effect-system.md#effect)
should be produced and handled during recovery?
Notice that, in particular, logging an input to the WAL is an `Effect`, but in
this case does it make sense to append to WAL inputs that were originally
replayed from that same WAL?

## Decision

This section is built atop the discussion of options presented in the
[Implementation](#implementation) section.

### Layers

A new `Effect` was defined in the [malachitebft-core-consensus][consensus-crate] crate
to request the logging of an input to the WAL:

```rust
    /// Append an entry to the Write-Ahead Log for crash recovery
    /// If the WAL is not at the given height, the entry should be ignored.
    /// 
    /// Resume with: [`resume::Continue`]`
    WalAppend(Ctx::Height, WalEntry<Ctx>, resume::Continue),
```

Notice that a `WalEntry` type defines the consensus inputs that are persisted,
as discussed in the [Inputs](#inputs-1) section below.

The Write-Ahead Log (WAL) is implemented as an
[Actor](./adr-002-node-actor.md#write-ahead-log-(wal)-actor)
and runs in its own thread, as part of the
[malachitebft-engine][engine-crate] crate.
The interaction between the consensus engine and the WAL routine uses a set of
messages (`enum WalMsg`), the most relevant being `Append` and `Flush`, `Reset`
and `StartedHeight`.
The first two messages are related to the [persistence](#persistence-1) of
inputs to the WAL;
`Reset` is related to [checkpoints](#reset);
and `StartedHeight` is actually related to [replaying inputs](#replay-1),
an operation that is conducted by consensus engine.

The handling of storage, files, formatting, versioning etc. is
implemented and tested in the [malachitebft-wal][wal-crate] crate.

#### Discussion

The persistence of inputs could in thesis be implemented in any of the three
abstract layers of Malachite.

The [malachitebft-core-state-machine][smr-crate] layer operates on complex
inputs, which are the only ones that produce relevant state transitions and
outputs.
However, the adopted definition of consistency (see [Context](#context))
requires the logging of single inputs.
For instance, if a process only receives `2f` matching `Precommit` messages,
there is no complex input to process or persist, nor produced output.
But the reception of an additional matching `Precommit` message leads to a
complex input and an observable state transition.
If the first `2f Precommit` messages are not persisted to the WAL, a recovering
process will not produce the same state transition when receiving the missing
`Precommit`.

The [malachitebft-driver](./adr-001-architecture.md#consensus-driver) layer do
handles single inputs and it is responsible for producing complex inputs to the
consensus state machine.
It would be the right layer to define what inputs to persist to the WAL and for
replaying persisted inputs upon recovery.
The driver, however, is not aware of the value dissemination mechanism.
It operates on `Proposal` inputs produced by the core consensus layer,
which are actually the combination of single inputs,
for instance, the reception of a `Proposal` message from the network
and of a matching `ProposedValue` input from the application.
In addition to that, the driver is _supposed_ to operate on _unsigned_ inputs,
previously validated by the underlying core consensus layer.
Signatures, however, have to be persisted and are part of outputs, such as the
decision of a value.

> Notice that the driver layer has the most advanced unit test primitives, that
> can be easily extended and for which Model-Based Testing instances can be
> produced.
> By not having WAL-related events at this layer, testing the WAL correctness
> becomes more challenging.

So, all in all, since the interaction with the WAL has to be managed by the
[malachitebft-core-consensus][consensus-crate] layer, this layer is aware of
the value dissemination protocol, and is able to handle signatures,
the logic interacting with the WAL was naturally implemented at this layer.

### Inputs

A new `WalEntry` type defines the consensus inputs that are persisted to the WAL:

```rust
pub enum WalEntry<Ctx: Context> {
    ConsensusMsg(SignedConsensusMsg<Ctx>), 
    ProposedValue(ProposedValue<Ctx>),
    Timeout(Timeout),
}
```

When compared to the [malachitebft-core-consensus][consensus-crate] crate's
`Input` type, notice:

- `Vote` and `Proposal` inputs were merged into `WalEntry::ConsensusMsg`
- `Propose` and `ProposedValue` inputs were merged into `WalEntry::ProposedValue`
- `TimeoutElapsed` is directly mapped to `WalEntry::Timeout`

So, all the [list of inputs](#inputs) that must be persisted were contemplated.
There are, however, important exceptions: some inputs produced by the
synchronization protocol (`SyncValueResponse`, which includes a `CommitCertificate`)
and by the liveness protocol (`PolkaCertificate` and `RoundCertificate`) are
not persisted.
They are all **Certificate**s, namely types that aggregate multiple `Vote` inputs.
By not persisting those inputs - namely, the `Vote` inputs not already persisted -
recovering processes may behave inconsistently (see [issue #1445][issue-certs]).

To conclude the list of `Input`s, `StartHeight` is not persisted to the WAL but
this input leads to either:

1. The [reset](#reset) of the WAL, namely clearing it to start a new height;
2. Or the [replay](#replay-1) of the WAL, from the collection of all logged `WalEntry` instances.

### Reset

The implementation adopted the strategy of producing [checkpoints](#checkpoints)
at every height, so that the WAL only contains inputs belonging to the latest
started height.
The `reset` operation of the `Log` type, implementing the WAL, receives a
height number, write it to the WAL header, and truncates the WAL.
It is invoked whenever `StartHeight` input is received from the application
for the first time for a height.

> It is also used in the case of the `RestartHeight` input, an _unsafe command_
> that instructs Malachite to _ignore_ the WAL's contents.

Note that all entries in the WAL must belong to the height the WAL is
configured for, written as part of its header.
This means that messages from future heights `H'> H`, where `H` is the highest
height for which a `StartHeight` input was processed, are not persisted.
They are only buffered in main memory and therefore lost upon restarts.
Which is safe, since they do not (immediately) produce state transitions.

### Replay

The replay of the WAL is triggered by the reception of `StartHeight` input from
the application for a height matching the WAL's height.
The `fetch_entries` method is used to iterate through the WAL and collect all stored
`WalEntry` instances, that are returned to the consensus engine.
If `WalEntry` instances are returned, the consensus engine is set to
`Phase::Recovering` and the persisted inputs are replayed by the `wal_replay` method.
This method reconstructs the associated consensus `Input`s and apply them using the
`process_input` method, which is the same used to process ordinary inputs.
The `Phase::Recovering` flag is only used for: (i) blocking the `WalAppend`
effect from appending again to the WAL inputs that are being replayed,
and (ii) blocking calls to the associated WAL's `flush()` method.

As a result, replayed inputs produce outputs, `Effect`s in the consensus engine
parlance, in the same way as ordinary inputs, the exception being only the
effects related to persisting inputs to the WAL.
In any case, the existence of the `Phase::Recovering` flag allows filtering out
behaviors, effects, an inputs that are not needed or redundant
in recovery mode - although it should be used very carefully.
Once replaying is done, the `Phase::Recovering` flag is cleared.
The WAL is not [reset](#reset-1) and the replayed inputs remain in the WAL for
the case in which the process crashes or is shut down again.
In addition, all inputs processed during normal operation are appended to the
WAL as usual.

### Persistence

The commands for persisting inputs to the WAL are `WalMsg::Append`, receiving a
`WalEntry` instance, and `WalMsg::Flush`.
This is in line with the previous described [persistence strategy](#persistence):
inputs can be appended in a non-blocking way using asynchronous writes, while
when an input produces outputs or relevant state transitions, a blocking
`flush()` call is needed, implementing a synchronous batch write.

Synchronous writes, using the `wal_flush` consensus engine method, are performed:

1. When a new round of consensus is started, via the `StartRound` effect;
2. When a consensus message is broadcast, via the `PublishConsensusMsg` effect;
3. When a value is finalized by the consensus, via the `Decide` effect.

Notice that synchronous writes are performed whenever the effect is
externalized, either to the application (1 and 3) or the network (2).

A current limitation of the persistence approach, however, is the fact that
calls to `wal_append` are also blocking, while they do not have to.
This is currently associated with error handling, but it would be good to find a
way to propagate and handle errors asynchronously ([#1435][issue-async]).

### Error Handling

The Write-Ahead Log is a crucial component for the operation of a consensus
process.
As a result, errors when attempting to append inputs to the WAL
**must be critical**.
At the moment, from this [commit](https://github.com/circlefin/malachite/commit/38f113f6c81da0af32a748718b2d87ab64e3a72f),
consensus hangs forever in case of any WAL operational error.

A second source of errors occurs during WAL replay and requires special attention: an error at a given entry implies that all subsequent WAL entries are corrupted.
Notice that crashes can happen at any time of the execution, including the
instant at which a WAL entry is being persisted to stable storage.
A crash at this point will likely render a suffix of the WAL corrupted.
This, however, is not a threat for safety because the outputs produced by the
associated state-machine transition are only emitted after the WAL is
[persisted](#persistence-1).
Since the append operation has not been concluded with success, the output was
not emitted, therefore it is just like it has never been produced.

In summary, as discussed in [issue #1434][issue-corrupt], corruptions at
the tail of the WAL should not produce a critical error.
The entries successfully decoded should be replayed, and the WAL should be
truncated to the end of the latest successfully decoded entry.
The last step is needed to enable further appending inputs to the WAL when the
recovery is concluded and new inputs are processed in "normal" operation.

## Status

Accepted

## Consequences

### Positive

* Malachite supports crash-recovery behaviour, preventing processes from equivocating
* No important changes were needed at core components of Tendermint implementation
* The consensus Engine implements a WAL actor that should suit most use cases
* Per-height WALs render the WAL file most of the time small, no need for rotations
* Costly synchronous writes to the WAL are reduced to the required scenarios

### Negative

* Persisting inputs to the WAL are in the critical path of consensus execution
* By having blocking WAL append calls, the implementation has a higher overhead than needed
* With the current design, testing the WAL operation is relatively complex
* With the current design, existing driver test units cannot evaluate the WAL

### Neutral

* The size of the WAL is limited to the number of inputs processed during a height of consensus

## References

* [spec: Consensus Write-Ahead Log (WAL) #469](https://github.com/circlefin/malachite/issues/469):
  initial discussion of requirements for the WAL
* [Consensus WAL may contain corrupted data #1434][issue-corrupt]
* [Consensus WAL should not block upon asynchronous writes #1435][issue-async]
* [Consensus WAL must store received certificates #1445][issue-certs]
* [spec: Candidate blocks (full proposed values) store #579][issue-values]
* [ADR 001: High Level Architecture for Tendermint Consensus Implementation in Rust](./adr-001-architecture.md)
* [ADR 003: Propagation of Proposed Values][adr-003]
* [ADR 004: Coroutine-Based Effect System for Consensus](./adr-004-coroutine-effect-system.md)

[smr-crate]: https://github.com/circlefin/malachite/tree/main/code/crates/core-state-machine
[driver-crate]: https://github.com/circlefin/malachite/tree/main/code/crates/core-driver
[consensus-crate]: https://github.com/circlefin/malachite/tree/main/code/crates/core-consensus
[engine-crate]: https://github.com/circlefin/malachite/tree/main/code/crates/engine
[wal-crate]: https://github.com/circlefin/malachite/tree/main/code/crates/wal
[pseudo-code]: https://github.com/circlefin/malachite/blob/main/specs/consensus/pseudo-code.md
[issue-corrupt]: https://github.com/circlefin/malachite/issues/1434
[issue-async]: https://github.com/circlefin/malachite/issues/1435
[issue-values]: https://github.com/circlefin/malachite/issues/579
[issue-certs]: https://github.com/circlefin/malachite/issues/1445
[adr-003]: ./adr-003-values-propagation.md
