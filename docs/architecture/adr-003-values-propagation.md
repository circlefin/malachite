# ADR 003: Propagation of Proposed Values

## Changelog

* 2025-03-04: Context and proposed values propagation Alternatives

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

The cost of the **Value Propagation** state,
in terms of latency and bandwidth consumption, 
evidently depends on the size of the proposed values.
While the cost of the **Value Decision** stage should be independent from the
size of the proposed values.

In Tendermint, the message that plays the role of **Value Propagation** is the
`PROPOSAL` message, as it carries the proposed value `v` for a round of consensus.
While the `PREVOTE` and `PRECOMMIT` messages are dedicated to the
**Value Decision** role and carry an identifier `id(v)` of a proposed value
`v`, or the special `nil` value - meaning "no value".
The function `id(v)` can be implemented in multiple ways, the most common of
which is by returning a hash of `v`.
This means that the size of the messages used in the **Value Decision** stage
is usually fixed and independent from the size of the proposed value.

## Decision

> This section explains all of the details of the proposed solution, including implementation details.
It should also describe affects / corollary items that may need to be changed as a part of this.
If the proposed change will be large, please also indicate a way to do the change to maximize ease of review.
(e.g. the optimal split of things to do between separate PR's)

## Status

> A decision may be "proposed" if it hasn't been agreed upon yet, or "accepted" once it is agreed upon. If a later ADR changes or reverses a decision, it may be marked as "deprecated" or "superseded" with a reference to its replacement.

Proposed

## Consequences

> This section describes the consequences, after applying the decision. All consequences should be summarized here, not just the "positive" ones.

### Positive

### Negative

### Neutral

## References

> Are there any relevant PR comments, issues that led up to this, or articles referenced for why we made the given design choice? If so link them here!

* [Tendermint consensus specification][consensus-spec]

[consensus-spec]: ../../specs/consensus/README.md
[consensus-code]: ../../specs/consensus/pseudo-code.md
[consensus-proposals]: ../../specs/consensus/overview.md#proposals
[consensus-votes]: ../../specs/consensus/overview.md#votes
[adr001]: ./adr-001-architecture.md
