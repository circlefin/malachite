## Message Handling

The [consensus design](./design.md) document describes the transitions produced
in the consensus state machine when certain conditions are met.
Most of the transitions are triggered by receiving messages that belong to the
**same height and round** as the current process' height and round.

However, due to the network asynchrony, different processes speeds, a process
may have to handle messages that belong to rounds or even heights different
from its current ones, as described in this document.

## Different rounds

Messages matching the current height and round of a process produce most of
the relevant events for the consensus state machine.
Messages from different rounds, however, also trigger relevant events.

This section assumes that a process is at round `r` of height `h` of
consensus, or in short, at round `(h, r)`.

### Previous rounds

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

As a result, a process needs to keep track of messages from previous
rounds to produce the enumerated events:

1. `PROPOSAL` messages should be maintained when a process moves to higher rounds,
   as well as new `PROPOSAL` messages from previous rounds should be stored.
   - Reason I: a `2f + 1` threshold of `⟨PRECOMMIT, h, r', id(v)⟩` messages
     could still be obtained, and an existing proposal message for `v` in the
     previous round `r' < r` enables the process to decide `v`.
   - Reason II: a `2f + 1` threshold of `⟨PRECOMMIT, h, r', id(v)⟩` messages
     was already obtained, but the proposal message for `v` at round `r'`
     is missing. Once received, the process can decide `v`.
2. `PREVOTE` messages should be maintained when a process moves to higher rounds,
   as well as new `PREVOTE` messages from previous rounds should be stored.
   - Reason I: a `PROPOSAL(h, r, v, vr)` with `0 ≤ vr < r` can be received in
     the current round, requiring an existing `2f + 1` threshold of `⟨PREVOTE, h, vr, id(v)⟩` messages.
   - Reason II: a `2f + 1` threshold of `⟨PREVOTE, h, vr, id(v)⟩` messages
     can still be obtained and unblock the processing of `PROPOSAL(h, r, v, vr)`
     received in the current round.
   - Observe that `PREVOTE` messages for `nil` do not need to be maintained for previous rounds.
3. `PRECOMMIT` messages should be maintained when a process moves to higher rounds,
   as well as new `PRECOMMIT` messages from previous rounds should be stored.
   - Reason I: a `2f + 1` threshold of `⟨PRECOMMIT, h, r', id(v)⟩` messages
     can be obtained, and there is a proposal message for `v` in round
     `r'`, leading the process to decide `v`.
   - Reason II: a `2f + 1` threshold of `⟨PRECOMMIT, h, r', id(v)⟩` messages
     can be obtained, but there is no proposal message for `v` in round
     `r'`. This enables Reason II of 1., i.e., receiving a late proposal.
   - Observe that `PRECOMMIT` messages for `nil` do not need to be maintained for previous rounds.

### Future rounds

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

According to the vote keeper [spec in Quint](./quint/votekeeper.qnt), the
first interpretation has been adopted.
Namely, the senders of both `PREVOTE` and `PRECOMMIT` messages of a round `r' > r`
are counted together towards the `f + 1` threshold.

### Attack vectors

In addition to the attack vectors induced by equivocating processes,
for [proposal messages](./overview.md#proposals) and
[vote messages](./overview.md#votes),
the need of storing message referring to previous or future rounds introduces
new attack vectors.

In the case of messages of [previous rounds](#previous-rounds), the attack
vectors are the same as for messages matching the current round, as the
process is supposed in any case to store all messages of previous rounds.
A possible mitigation is the observation that vote messages for `nil` have no
use when they refer to previous rounds.

In the case of messages of [future rounds](#future-rounds) `r' > r`,
in addition to tracking message senders to enable round skipping,
a process _must_ store the (early) received messages so that they can be
processed and produce relevant events once the process starts the future
round `r'`.
This constitutes an important attack vector, as Byzantine processes could
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
   from a future round `r' > r` once the process reaches round `r'`.
   Since processes keep track of messages of both the current and previous
   rounds, they should be able to transmit those messages to their lagging peers.

## Different heights

Heights in Tendermint consensus algorithm are communication-closed.
This means that if a process is at height `h`, messages from either `h' < h`
(past) or `h' > h` (future) heights have no effect on the operation of height `h`.

However, due to asynchrony, different processes can be at different heights.
More specifically, assuming a lock-step operation for heights (i.e., a
process only starts height `h + 1` once height `h` is decided), some
processes can be trying to decide a value for height `h` while others have
already transitioned to heights `h' > h`.

An open question is whether the consensus protocol should be in charge of
handling lagging processes.
This is probably easier to be implement by a separate or auxiliary component,
which implements a syncing protocol.

### Past heights

The consensus state machine is not affected by messages from past heights.
However, the reception of such messages from a peer indicates that the peer may
lagging behind in the protocol, and need to be caught up.

To catchup a peer that is behind in the protocol (previous heights) it would be
enough to provide the peer with the `Proposal` for the decided value `v` and
a `2f + 1` threshold of `Precommit` messages of the decision round for `id(v)`.
These messages, forming a _decision certificate_, should be stored for a given
number of previous heights.

### Future heights

The consensus state machine is not able to process message from future heights
in a proper way, as the process set for for a future height may not be known
until the future height is started.
However, once the process reaches the future height, messages belonging to
that height that were early received are **required** for proper operation.

An additional complication when handling messages from future heights is that,
contrarily to what happens with messages of [future rounds](#future-rounds),
there is no mechanism that allows the process to switch to the future height
when it receives a given set of messages from that height.
In fact, considering the lock-step operation of the consensus algorithm, a
node can only start height `h` once height `h - 1` is decided.
Moreover, messages of future heights `h' > h` do not enable, in any way, a
node to reach a decision in its current height `h`.

### Attack vectors

In addition to the attack vectors induced by equivocating processes,
for [proposal messages](./overview.md#proposals) and
[vote messages](./overview.md#votes),
the need of storing message referring to previous or future heights introduces
new attack vectors.

If messages from [previous heights](#past-heights) from a peer trigger a different node to
execute procedures for trying to catch up that peer, a Byzantine peer may
indefinitely claim to be stuck in a previous height, or that it is behind by
several heights.
In both cases the node will consume resources to catchup a peer that possibly
does not need to be caught up.

The fact that a process needs to store messages from [future heights](#future-heights),
so that they can be processed and produce relevant events once the process
eventually starts the corresponding heights,
constitutes a very important attack vector, as Byzantine processes could
broadcast messages referring to an arbitrary number of future heights.

There is no trivial solution for preventing the attack derived from the need of
storing messages of future heights.
However, the following approaches, individually or combined, can mitigate the
effects of this attack:

1. Buffer messages for a limited number of future heights, say heights
   `h'` where `h < h' ≤ h_max`.
2. Assume that the communication subsystem (p2p) is able to retrieve messages
   from future heights `h' > h` once the process reaches height `h'`.
   Notice that this option implies that processes keep a minimal set of
   consensus messages from [previous heights](#past-heights) so that to enable
peers lagging behind to decide a past height.
