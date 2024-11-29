# Malachite Documentation

Malachite is an implementation of the [Tendermint consensus algorithm][arxiv] in Rust.
It comes together with an executable specification in [Quint][quint-spec]. We use
model-based testing to make sure that the implementation corresponds to the
specification.

Tendermint consensus algorithm works by a set of validator nodes exchanging messages over a
network, and the local consensus instances act on the incoming messages if
certain conditions are met (e.g., if a threshold number of specific messages is
received a state transition should happen).
The architecture of Malachite separates:

- counting messages in a *vote keeper* ([Quint][quint-votekeeper]),
- creating consensus inputs in a *driver* ([Quint][quint-driver]), e.g., if a threshold is reached
- doing the state transition depending on the consensus input in the *state machine* ([Quint][quint-sm])

A detailed executable specification of these functionalities are given in Quint.
In this (English) document we discuss some underlying principles, namely,

- [Message handling](#message-handling): How to treat incoming messages. Which messages to store,
and on what conditions to generate consensus inputs.

- [Round state machine](#round-state-machine): How to change state depending on the
current state and a consensus input.

- [Misbehavior detection and handling](../../consensus/misbehavior.md): How Faulty nodes can misbehave, how it can be detected, and how objective proof of misbehavior can be computed that can be soundly used to incentivize nodes to behave nicely (penalties, slashing, etc. are not in the scope of the consensus engine, and will thus not be discussed here).

## Message Handling

Most of this content has been moved into the [Consensus algorithm overview](../../consensus/README.md#messages)

### Different rounds

Messages matching the current height and round of a validator produce most of
the relevant events for the consensus state machine.
Messages from different rounds, however, also trigger relevant events.

This section assumes that a validator is at round `r` of height `h` of
consensus, or in short, at round `(h, r)`.

#### Previous rounds

The consensus state machine has events requiring messages from previous rounds
`(h, r')` with `r' < r`:

- `PREVOTE` messages may be required to produce a Proof of Lock (POL or Polka) for a
  value `v` needed for accepting a `PROPOSAL(h, r, v, vr)` message, with
  `0 ≤ vr < r`, of the current round (L28).
  - A Polka for `v` at round `vr` is a `2f + 1` threshold of `⟨PREVOTE, h, vr, id(v)⟩` messages.
- `PROPOSAL` messages from previous rounds can be required to decide a value
  (L49), see more details below.
- `PRECOMMIT` messages can produce a `2f + 1` threshold of `⟨PRECOMMIT, h, r', id(v)⟩`
   messages which, together with a `PROPOSAL(h, r', v, *)` message,
  leads to the decision of `v` at round `r'` (L49).

As a result, a validator needs to keep track of messages from previous
rounds to produce the enumerated events:

1. `PROPOSAL` messages should be maintained when a validator moves to higher rounds,
   as well as new `PROPOSAL` messages from previous rounds should be stored.
   - Reason I: a `2f + 1` threshold of `⟨PRECOMMIT, h, r', id(v)⟩` messages
     could still be obtained, and an existing proposal message for `v` in the
     previous round `r' < r` enables the validator to decide `v`.
   - Reason II: a `2f + 1` threshold of `⟨PRECOMMIT, h, r', id(v)⟩` messages
     was already obtained, but the proposal message for `v` at round `r'`
     is missing. Once received, the validator can decide `v`.
2. `PREVOTE` messages should be maintained when a validator moves to higher rounds,
   as well as new `PREVOTE` messages from previous rounds should be stored.
   - Reason I: a `PROPOSAL(h, r, v, vr)` with `0 ≤ vr < r` can be received in
     the current round, requiring an existing `2f + 1` threshold of `⟨PREVOTE, h, vr, id(v)⟩` messages.
   - Reason II: a `2f + 1` threshold of `⟨PREVOTE, h, vr, id(v)⟩` messages
     can still be obtained and unblock the processing of `PROPOSAL(h, r, v, vr)`
     received in the current round.
   - Observe that `PREVOTE` messages for `nil` do not need to be maintained for previous rounds.
3. `PRECOMMIT` messages should be maintained when a validator moves to higher rounds,
   as well as new `PRECOMMIT` messages from previous rounds should be stored.
   - Reason I: a `2f + 1` threshold of `⟨PRECOMMIT, h, r', id(v)⟩` messages
     can be obtained, and there is a proposal message for `v` in round
     `r'`, leading the validator to decide `v`.
   - Reason II: a `2f + 1` threshold of `⟨PRECOMMIT, h, r', id(v)⟩` messages
     can be obtained, but there is no proposal message for `v` in round
     `r'`. This enables Reason II of 1., i.e., receiving a late proposal.
   - Observe that `PRECOMMIT` messages for `nil` do not need to be maintained for previous rounds.

#### Future rounds

The consensus state machine requires receiving and processing messages from
future rounds `(h, r')` with `r' > r` for enabling the _round skipping_ mechanism.
This mechanism is defined in the pseudocode as follows:

```
55: upon f + 1 ⟨∗, hp, round, ∗, ∗⟩ with round > roundp do
56:   StartRound(round)
```

The definition is ambiguous and the event triggering round skipping can be
interpreted in two main ways:

1. Messages of any type and round `r' > r` are received so that the
   `f + 1` threshold is reached.
2. Messages of a given type and round `r' > r` are received so that the
   `f + 1` threshold is reached.

Since proposal messages for a round have a single sender, the round's proposer,
in both interpretations the vote messages are the ones that really count
towards the `f + 1` threshold.
The question then is whether we count the senders of `PREVOTE` and `PRECOMMIT`
messages separately (i.e., one set per vote type) or together.

According to the vote keeper [spec in Quint][quint-votekeeper], the
first interpretation has been adopted.
Namely, the senders of both `PREVOTE` and `PRECOMMIT` messages of a round `r' > r`
are counted together towards the `f + 1` threshold.

#### Attack vectors

In addition to the attack vectors induced by equivocating validators,
for [proposal messages](#proposals) and [vote messages](#counting-votes),
the need of storing message referring to previous or future rounds introduces
new attack vectors.

In the case of messages of [previous rounds](#previous-rounds), the attack
vectors are the same as for messages matching the current round, as the
validator is supposed in any case to store all messages of previous rounds.
A possible mitigation is the observation that vote messages for `nil` have no
use when they refer to previous rounds.

In the case of messages of [future rounds](#future-rounds) `r' > r`,
in addition to tracking message senders to enable round skipping,
a validator _must_ store the (early) received messages so that they can be
processed and produce relevant events once the validator starts the future
round `r'`.
This constitutes an important attack vector, as Byzantine validators could
broadcast messages referring to an arbitrary number of future rounds.

There is no trivial solution for preventing the attack derived from the need of
storing messages of future rounds.
However, the following approaches, individually or combined, can mitigate the
effects of this attack:

1. Store messages only for a limited number future rounds, say future rounds
   `r'` where `r < r' ≤ r_max`.
   - For instance,  CometBFT only tracks messages of a single future round,
     i.e., `r_max = r + 1`.
2. Assume that the communication subsystem (p2p) is able to retrieve messages
   from a future round `r' > r` once the validator reaches round `r'`.
   Since validators keep track of messages of both the current and previous
   rounds, they should be able to transmit those messages to their lagging peers.

### Different heights

Heights in Tendermint consensus algorithm are communication-closed.
This means that if a validator is at height `h`, messages from either `h' < h`
(past) or `h' > h` (future) heights have no effect on the operation of height `h`.

However, due to asynchrony, different validators can be at different heights.
More specifically, assuming a lock-step operation for heights (i.e., a
validator only starts height `h + 1` once height `h` is decided), some
validators can be trying to decide a value for height `h` while others have
already transitioned to heights `h' > h`.

An open question is whether the consensus protocol should be in charge of
handling lagging validators.
This is probably easier to be implement by a separate or auxiliary component,
which implements a syncing protocol.

#### Past heights

The consensus state machine is not affected by messages from past heights.
However, the reception of such messages from a peer indicates that the peer may
lagging behind in the protocol, and need to be caught up.

To catchup a peer that is behind in the protocol (previous heights) it would be
enough to provide the peer with the `Proposal` for the decided value `v` and
a `2f + 1` threshold of `Precommit` messages of the decision round for `id(v)`.
These messages, forming a _decision certificate_, should be stored for a given
number of previous heights.

#### Future heights

The consensus state machine is not able to process message from future heights
in a proper way, as the validator set for for a future height may not be known
until the future height is started.
However, once the validator reaches the future height, messages belonging to
that height that were early received are **required** for proper operation.

An additional complication when handling messages from future heights is that,
contrarily to what happens with messages of [future rounds](#future-rounds),
there is no mechanism that allows the validator to switch to the future height
when it receives a given set of messages from that height.
In fact, considering the lock-step operation of the consensus algorithm, a
node can only start height `h` once height `h - 1` is decided.
Moreover, messages of future heights `h' > h` do not enable, in any way, a
node to reach a decision in its current height `h`.

#### Attack vectors

In addition to the attack vectors induced by equivocating validators,
for [proposal messages](#proposals) and [vote messages](#counting-votes),
the need of storing message referring to previous or future heights introduces
new attack vectors.

If messages from [previous heights](#previous-heights) from a peer trigger a different node to
execute procedures for trying to catch up that peer, a Byzantine peer may
indefinitely claim to be stuck in a previous height, or that it is behind by
several heights.
In both cases the node will consume resources to catchup a peer that possibly
does not need to be caught up.

The fact that a validator needs to store messages from [future heights](#future-heights),
so that they can be processed and produce relevant events once the validator
eventually starts the corresponding heights,
constitutes a very important attack vector, as Byzantine validators could
broadcast messages referring to an arbitrary number of future heights.

There is no trivial solution for preventing the attack derived from the need of
storing messages of future heights.
However, the following approaches, individually or combined, can mitigate the
effects of this attack:

1. Buffer messages for a limited number of future heights, say heights
   `h'` where `h < h' ≤ h_max`.
2. Assume that the communication subsystem (p2p) is able to retrieve messages
   from future heights `h' > h` once the validator reaches height `h'`.
   Notice that this option implies that validators keep a minimal set of
   consensus messages from [previous heights](#past-heights) so that to enable
peers lagging behind to decide a past height.


## Round state machine

The consensus state-machine operates on complex `Event`s that reflect the
reception of one or multiple `Message`s, combined with state elements and the
interaction with other modules.

The state machine represents the operation of consensus at a single `Height(h)` and `Round(r)`.
The diagram below offers a visual representation of the state machine. It shows the input events, using green for simple inputs (e.g. timeouts, proposal)
and red for the complex events (e.g. `ProposalAndPolkaCurrent` is sent to the state machine when a valid proposal and a polka of prevotes have been received).
The actions are shown in italics (blue) and the output messages are shown in blue.

![Consensus SM Diagram](../assets/sm_diagram.jpeg)

The set of states can be summarized as:

- `Unstarted`
  - Initial state
  - Can be used to store messages early received for this round
  - In the algorithm when `roundp < r`, where `roundp` is the node's current round
- InProgress (`Propose`, `Prevote`, `Precommit`)
  - Actual consensus single-round execution
  - In the algorithm when `roundp == r`
- `Commit`
  - Final state for a successful round

### Exit transitions

The table below summarizes the major state transitions in the `Round(r)` state machine.
The transactions from state `InProgress` consider that node can be at any of
the `Propose`, `Prevote`, `Precommit` states.
The `Ref` column refers to the line of the pseudocode where the events can be found.

| From       | To         | Ev Name                      | Event  Details                                                    | Action                            | Ref |
| ---------- |------------|------------------------------|-------------------------------------------------------------------|-----------------------------------| --- |
| InProgress | InProgress | PrecommitAny                 | `2f + 1 ⟨PRECOMMIT, h, r, *⟩` <br> for the first time             | schedule `TimeoutPrecommit(h, r)` | L47 |
| InProgress | Unstarted  | TimeoutPrecommit             | `TimeoutPrecommit(h, r)`                                          | `next_round(r+1)`                 | L65 |
| InProgress | Unstarted   | SkipRound(r')                | `f + 1 ⟨*, h, r', *, *⟩` with `r' > r`                            | `next_round(r')`                  | L55 |
| InProgress | Commit     | ProposalAndPrecommitValue(v) | `⟨PROPOSAL, h, r', v, *⟩` <br> `2f + 1 ⟨PRECOMMIT, h, r', id(v)⟩` | `commit(v)`                       | L49 |

### InProgress round

The table below summarizes the state transitions within the `InProgress` state
of the `Round(r)` state machine.
The following state transitions represent the core of the consensus algorithm.
The `Ref` column refers to the line of the pseudocode where the events can be found.

| From      | To        | Event                                  | Details                                                                                | Actions and Return                                                                                    | Ref |
|-----------|-----------|----------------------------------------|----------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------|-----|
| Unstarted  | Propose   | NewRound(proposer)                     | `StartRound` with `proposer(h, r) = p`                                                 | **async `getValue()` and schedule `TimeoutPropose(h, r)`**                                                | L19 |
| Unstarted  | Propose   | NewRound(non-proposer)                 | `StartRound` with `proposer(h, r) != p` (optional restriction)                         | schedule `TimeoutPropose(h, r)`                                                                       | L21 |
| **Propose**   | **Propose**   | **ProposeValue(v)**                        | `getValue()` returned                                                                  | broadcast `⟨PROPOSAL, h, r, v, validRound⟩`                                               | L19 |
| Propose   | Prevote   | Proposal(v, -1)                        | `⟨PROPOSAL, h, r, v, −1⟩`                                                              | broadcast `⟨PREVOTE, h, r, {id(v), nil}⟩`                                                             | L23 |
| Propose   | Prevote   | **InvalidProposal**(v, -1)                 | `⟨PROPOSAL, h, r, v, −1⟩`                                                              | broadcast `⟨PREVOTE, h, r, nil⟩`                                                                      | L32 |
| Propose   | Prevote   | ProposalAndPolkaPrevious(v, vr)        | `⟨PROPOSAL, h, r, v, vr⟩` <br> `2f + 1 ⟨PREVOTE, h, vr, id(v)⟩` with `vr < r`          | broadcast `⟨PREVOTE, h, r, {id(v), nil}⟩`                                                             | L30 |
| Propose   | Prevote   | **InvalidProposalAndPolkaPrevious**(v, vr) | `⟨PROPOSAL, h, r, v, vr⟩` <br> `2f + 1 ⟨PREVOTE, h, vr, id(v)⟩` with `vr < r`          | broadcast `⟨PREVOTE, h, r, nil⟩`                                                                      | L32 |
| Propose   | Prevote   | TimeoutPropose                         | `TimeoutPropose(h, r)`                                                                 | broadcast `⟨PREVOTE, h, r, nil⟩`                                                                      | L57 |
| Prevote   | Prevote   | PolkaAny                               | `2f + 1 ⟨PREVOTE, h, r, *⟩` <br> for the first time                                    | schedule `TimeoutPrevote(h, r)⟩`                                                                      | L34 |
| Prevote   | Precommit | ProposalAndPolkaCurrent(v)             | `⟨PROPOSAL, h, r, v, ∗⟩` <br> `2f + 1 ⟨PREVOTE, h, r, id(v)⟩` <br> for the first time  | update `lockedValue, lockedRound, validValue, validRound`,<br /> broadcast `⟨PRECOMMIT, h, r, id(v)⟩` | L36 |
| Prevote   | Precommit | PolkaNil                               | `2f + 1 ⟨PREVOTE, h, r, nil⟩`                                                          | broadcast `⟨PRECOMMIT, h, r, nil⟩`                                                                    | L44 |
| Prevote   | Precommit | TimeoutPrevote                         | `TimeoutPrevote(h, r)`                                                                 | broadcast `⟨PRECOMMIT, h, r, nil⟩`                                                                    | L61 |
| Precommit | Precommit | PolkaValue(v)                          | `⟨PROPOSAL, h, r, v, ∗⟩` <br>  `2f + 1 ⟨PREVOTE, h, r, id(v)⟩` <br> for the first time | update `validValue, validRound`                                                                       | L36 |

The ordinary operation of a round of consensus consists on the sequence of
round steps `Propose`, `Prevote`, and `Precommit`, represented in the table.
The conditions for concluding a round of consensus, therefore for leaving the
`InProgress` state, are presented in the previous subsection.

#### Validity Checks

The pseudocode of the algorithm includes validity checks for the messages. These checks have been moved out of the state machine and are now performed by the `driver` module.
For this reason:
- `L22` is covered by `Proposal(v, -1) and `InvalidProposal(v, -1)`
- `L28` is covered by `ProposalAndPolkaPrevious(v, vr)` and `InvalidProposalAndPolkaPrevious(v, vr)`
- `L36` and `L49` are only called with valid proposal

TODO - show the full algorithm with all the changes

#### Asynchronous getValue() and ProposeValue(v)

The original algorithm is modified to allow for asynchronous `getValue()`. The details are described below.

<table>
<tr>
<th>arXiv paper</th>
<th>Async getValue()</th>
</tr>

<tr >
<td>

```
function StartRound(round) {
 round_p ← round
 step_p ← propose
 if proposer(h_p, round_p) = p {
  if validValue_p != nil {
   proposal ← validValue_p



  } else {
   proposal ← getValue()

  }


  broadcast ⟨PROPOSAL, h_p, round_p, proposal, validRound_p⟩
 } else {
  schedule OnTimeoutPropose(h_p,round_p) to
   be executed after timeoutPropose(round_p)
 }
}
```

</td>

<td>

```
function StartRound(round) {
 round_p ← round
 step_p ← propose
 if proposer(h_p, round_p) = p {
  if validValue_p != nil {
   proposal ← validValue_p

   broadcast ⟨PROPOSAL, h_p, round_p, proposal, validRound_p⟩

  } else {
   getValue() // async
   schedule OnTimeoutPropose(h_p,round_p) to
     be executed after timeoutPropose(round_p)
  }


 } else {
  schedule OnTimeoutPropose(h_p,round_p) to
   be executed after timeoutPropose(round_p)
 }
}
```

</td>
</tr>
</table>

- New Rule added

<table>
<tr>
<th>arXiv paper</th>
<th>Async getValue()</th>
</tr>

<tr>
<td>

```
```

</td>

<td>

```
upon PROPOSEVALUE (h_p, round_p, v) {
   proposal ← v
   broadcast ⟨PROPOSAL, h_p, round_p, proposal, -1⟩
}
```

</td>
</tr>
</table>


### Notes

Most of the state transitions represented in the previous tables consider message and
events referring to the node's current round `r`.
In the pseudocode this current round of a node is referred as `round_p`.

There are however exceptions that have to be handled properly:
- the transition `L28` requires the node to have access to `PREVOTE` messages from a previous round `r' < r`.
- the transition `L49` requires the node to have access to `PRECOMMIT` messages from different round `r' != r`.
- the transition `L55` requires the node to have access to all messages from a future round `r' > r`.

## References

* ["The latest gossip on BFT consensus"][arxiv], by _Buchman, Kwon, Milosevic_. 2018.

[arxiv]: https://arxiv.org/pdf/1807.04938.pdf
[quint-spec]: ../../quint/README.md
[quint-votekeeper]: ../../quint/specs/votekeeper.qnt
[quint-driver]: ../../quint/specs/driver.qnt
[quint-sm]: ../../quint/specs/consensus.qnt
