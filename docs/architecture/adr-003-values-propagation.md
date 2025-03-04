# ADR 003: Propagation of Proposed Values

## Changelog

* 2025-03-04: First draft

## Context

> This section contains all the context one needs to understand the current state, and why there is a problem. It should be as succinct as possible and introduce the high level idea behind the solution.

Malachite implements a consensus algorithm, [Tendermint][consensus-spec],
that receives as input, for each instance or height of consensus,
a number of proposed values and should output a single decided value,
among the proposed ones.

There is no assumptions regarding what a **value** is or represents:
its semantics is defined by whatever software uses consensus to propose and
decide values, which from now on we generically refer to as the *application*.
For example, blockchain applications provide as input to consensus blocks to be
appended to the blockchain.

This ADR is motivated by the fact that a consensus algorithm should actually
implement two main roles:

- **Value Propagation**: proposed values should be transmitted to all consensus
  processes;
- **Value Decision**: a single value, among the possibly multiple proposed
  values, must be decided.

As discussed in the [Value Propagation](#value-propagation) section,
this role tends to be the most expensive, in terms of latency and bandwidth
consumption, of the algorithm.
The [Tendermint](#tendermint) section discusses how these roles are implemented
by Tendermint; readers familiar with the algorithm may skip it.

### Tendermint

Tendermint implements both the **Value Propagation** and **Value Decision** roles.
Its [pseudo-code][consensus-code], however, reveals that there is already an
abstract distinction between the two roles.
More specifically, Tendermint is organized into heights and rounds; each round
has three round steps (`propose`, `prevote`, and `precommit`), each round step
having an associated message type:

- `PROPOSAL` messages carry a proposed value `v` and its main role is of
  **Value Propagation**.
  They are referred to as [**proposals**][consensus-proposals] and are
  broadcast by the `proposer` of a round in the `propose` round step;
- `PREVOTE` and `PRECOMMIT` messages carry an identifier `id(v)` of a proposed
  value `v`, or the special value `nil` meaning "no value", and play the role
  of **Value Decision**.
  They are referred to as [**votes**][consensus-votes], and are broadcast by
  all processes, respectively, in the `prevote` and `precommit` round steps.

Note that `PROPOSAL` messages have also a role on **Value Decision**, as they
carry consensus-related fields that are validated by processes and have
implications in the algorithm.
The point is that they are the only messages with **Value Dissemination** role,
so that the `propose` round step is where the dissemination of values takes
place.

A second remark is that if a vote issued by a process carries `id(v)`, then the
process must have received a `PROPOSAL` message carrying value `v`.
In other words, the **Value Decision** stage of a round can only succeed in
deciding a value `v` if the associated **Value Propagation** stage has also
been successful in delivering the proposed value `v` to all (correct) processes.

In fact, every state-transition predicate in the [pseudo-code][consensus-code]
that may lead to a successful round of consensus, i.e. to the decision of a
value `v`, requires the **Value Propagation** stage to have been successful,
that is, includes the condition:

```
XX: upon ⟨PROPOSAL, h_p, r, v, vr⟩ from proposer(h_p, r)
```

where `h_p` is current height of consensus process `p`, `r` is a round
(typically `p`'s current round `round_p`), and `vr` is a previous valid round
number `vr < r`, which is only relevant during the `propose` round step.

### Value Propagation

The propagation of proposed values in Tendermint happens in the first step, the
`propose` step, of a consensus round.
The `proposer` of the round selects a value `v` to propose and includes it in a
`PROPOSAL` message that is send to every process.
In the ordinary case, the proposed value is the input for the process,
retrieved via the `getValue()` function ([pseudo-code][consensus-code] line 18).

The propagation latency for the `PROPOSAL` message is directly associated to
the byte size of the proposed value `v`.
This is in contrast with the other consensus messages, `PREVOTE` and
`PRECOMMIT`, that carry an `id(v)`, which is an identifier of a proposed value
`v`, that is expected to be smaller than `v` and essentially of fixed byte
size. As a result, the latency of the `prevote` and `precommit` round steps
should be fairly constant.

Thus, as proposed values `v` get larger, the `propose` step becomes the most
expensive of a consensus round.
It is therefore natural to devise strategies to render the implementation of
the **Value Propagation** role more efficient.

Here it is important to notice that the
[High Level Architecture for Tendermint Consensus Implementation (ADR 001)][adr001]
already enables distinct implementations for the broadcast of `PROPOSAL` messages,
that propagate potentially _large_ (variable-size) proposed values `v`,
and for vote messages (`PREVOTE` and `PRECOMMIT`),
carrying _small_ (fixed-size) value identifiers `id(v)`:

```rust
/// Output of the round state machine.
pub enum Output<Ctx>
    where Ctx: Context,
{
    [...]

    /// Broadcast the proposal.
    Proposal(Ctx::Proposal),

    /// Broadcast the vote.
    Vote(Ctx::Vote),

    [...]
}
```

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
