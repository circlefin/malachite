# ADR 003: Propagation of Proposed Values

## Changelog

* 2025-03-18: Context and description of the problem

## Context

> This section contains all the context one needs to understand the current state, and why there is a problem. It should be as succinct as possible and introduce the high level idea behind the solution.

Malachite implements a consensus algorithm, [Tendermint][consensus-spec],
that receives as input, for each instance or height of consensus,
a number of proposed values and should output a single decided value,
among the proposed ones.

There is no assumptions regarding what a **value** is or represents:
its semantics is defined by whatever software uses consensus to propose and
decide values - which from now it is generically referred to as the *application*.
For example, blockchain applications provide as input to consensus blocks to be
appended to the blockchain.

There is also no assumption, at consensus level, regarding the **size** of
proposed values: a priori, they can have an arbitrary byte size.
The application, however, is expected to define a maximum byte size for
proposed values and to configure consensus accordingly - of particular
relevance here is the configured duration for timeouts.

In particular when the size of proposed values is a factor,
it is important to highlight that the implementation of a consensus algorithm
actually comprises two stages:

- **Value Propagation**: proposed values should be transmitted to all consensus
  processes;
- **Value Decision**: a single value, among the possibly multiple proposed and
  successfully propagated values, must be decided.

The cost of the **Value Propagation** stage,
in terms of latency and bandwidth consumption, 
evidently depends on the size of the proposed values.
While the cost of the **Value Decision** stage should be independent from the
size of the proposed values, and essentially constant.

In Tendermint, the message that plays the role of **Value Propagation** is the
`PROPOSAL` message.
It is broadcast by the proposer of a round of consensus and carries the
proposed value `v`, defined by the proposer process.

The **Value Decision** role is played by `PREVOTE` and `PRECOMMIT`, generically
called votes.
A vote carries either an identifier `id(v)` of a proposed value `v`, or the
special `nil` value.
The function `id(v)` should provide a short representation of a value `v`.
It can be implemented in multiple ways, the most common of which is by
returning a fixed byte-size hash of `v`.

## Alternatives

This section presents a (possibly not comprehensive) list of approaches to
handle **Value Propagation** for consensus protocols in general, and for
Tendermint in particular, discussing the pros and cons of each of them.

### Consensus by Value

In this approach, the consensus implementation plays both the
**Value Propagation** and **Value Decision** roles.

This means that a `PROPOSAL(h, r, v, vr)` consensus message broadcast by a
process carries its proposed value `v`.
Other processes learn the proposed value `v` when they receive the associated
`PROPOSAL` message.

As previously discussed, the vote messages (`PREVOTE` and `PRECOMMIT`) do not
carry the proposed value `v`, but a short representation `id(v)` of it.
A process cannot sign a vote for `id(v)` if it does not know the value `v`.
So receiving `PROPOSAL` message carrying the value `v` is a requirement for
signing a vote for `id(v)`.

If the round of consensus is successful, the value `v` carried by the round's
`PROPOSAL` message is the value delivered to the application as the decision
for that height of consensus.

**TODO**: how Malachite supports this approach.
In particular, how the application can disseminate proposed values in an
efficient way.

### Consensus by Reference

In this approach, the application is responsible for implementing the
**Value Propagation** stage,
while the consensus algorithm implements **Value Decision** stage.

This means that a `PROPOSAL(h, r, v, vr)` consensus message broadcast by a
process does not carry the value proposed by this process.
The value `v` ordered by the consensus algorithm is instead a reference, a
description, or an identifier of the value actually proposed by the process,
whose propagation is a responsibility of the application.

So if a process wants to propose a value `V` using this approach, it has:
(i) to propagate `V` to all processes, then (ii) produce a reference `v` of the
proposed value `V` and provide `v` to the consensus implementation.
On the receive side, a process that receives a `PROPOSAL` carrying `v` should
ensure that the referenced value `V` has been received as well.
Only in this case, the process can deliver the `PROPOSAL` for `v` to the
consensus implementation.

Since the values that are proposed and decided by consensus are references to
actually proposed values, `v` is expected to be a short representation of `V`.
For this reason, the optimization of having vote messages carrying `id(v)`
instead of `v` becomes pretty much irrelevant.

If the round of consensus is successful, the reference `v` carried by the
round's `PROPOSAL` message is the value delivered by the consensus
implementation as the decision for that height.
But the actual decision value for that height of consensus is `V`, the value
referenced by `v`.
It is `V` and not `v` that should be delivered to the application.

Notice, however, that a value can only be decided by the consensus
implementation if a `PROPOSAL` message carrying that value was previously
decided.
As already mentioned, in this approach, a `PROPOSAL` message carrying a
reference `v` can only be delivered to the consensus implementation if the
referenced value `V` is known by the process.
Therefore, a process where `v` is decided by the consensus implementation
should be able to deliver the actual proposed value `V` to the application.

**TODO**: under the hood, Malachite implements this approach.
But we probably need to offer an example of this approach, for instance, when
values/payloads are disseminated independently and consensus is used to order
the disseminated values, by receiving as `v` identifiers of disseminated values.

## Current Design

**TODO**: the three modes of execution.

The Proposals Streaming protocol.

## Decision

> This section explains all of the details of the proposed solution, including
> implementation details.
It should also describe affects / corollary items that may need to be changed
as a part of this.  If the proposed change will be large, please also indicate
a way to do the change to maximize ease of review.  (e.g. the optimal split of
things to do between separate PR's)

## Status

> A decision may be "proposed" if it hasn't been agreed upon yet, or "accepted"
> once it is agreed upon. If a later ADR changes or reverses a decision, it may
> be marked as "deprecated" or "superseded" with a reference to its
> replacement.

Proposed

## Consequences

> This section describes the consequences, after applying the decision. All
> consequences should be summarized here, not just the "positive" ones.

### Positive

### Negative

### Neutral

## References

> Are there any relevant PR comments, issues that led up to this, or articles
> referenced for why we made the given design choice? If so link them here!

* [Tendermint consensus specification][consensus-spec]

[consensus-spec]: ../../specs/consensus/README.md
[consensus-code]: ../../specs/consensus/pseudo-code.md
[consensus-proposals]: ../../specs/consensus/overview.md#proposals
[consensus-votes]: ../../specs/consensus/overview.md#votes
[adr001]: ./adr-001-architecture.md
