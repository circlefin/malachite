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
and how the consensus WAL could be implemented.

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

TODO: discuss which inputs to persist

### Write Mode

TODO: synchronous versus asynchronous writes

### Replay

TODO: how to replay logged inputs, how to handle the produced outputs during recovery

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
