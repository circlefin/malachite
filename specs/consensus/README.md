# Consensus Algorithm

Malachite adopts the Tendermint consensus algorithm from the paper
["The latest gossip on BFT consensus"](https://arxiv.org/abs/1807.04938)
([PDF](https://arxiv.org/pdf/1807.04938)), by Ethan Buchman, Jae Kwon,
and Zarko Milosevic, last revised in November 2019.

The **pseudo-code** of the algorithm, referenced several times in this
specification, is the Algorithm in page 6, that for simplicity and easy
reference is copied into the [pseudo-code.md][pseudo-code] file.

## Overview

> TODO: the content of this big section should be moved into a `./overview.md`
> file, linked in `README.md`.

A consensus algorithm is run by a (previously defined) set of **processes**[^1], some
of which may fail, that **propose** values and guarantees that eventually all
correct processes **decide** the same value, among the proposed ones.

Tendermint is a Byzantine Fault-Tolerant (BFT) consensus algorithm, which means
that it is designed to tolerate the most comprehensive set of faulty
behaviours.
If fact, a Byzantine process is a faulty process that can operate arbitrarily, in
particular it can, deliberately or not, disregard the rules imposed by the
algorithm.
Tendermint can solve consensus as long as **less than one third of the
processes are Byzantine**, i.e., operate arbitrarily.

Tendermint assumes that all consensus messages contain a **digital signature**.
This enables a process receiving a message to authenticate its sender and
content.
Byzantine nodes are assumed to not to be able to break digital signatures,
that is, they cannot forge messages and impersonate correct senders
(a.k.a. non-masquerading).

### Heights

The algorithm presented in the [pseudo-code][pseudo-code] represent the
operation of an instance of consensus in a process `p`.
Each instance or **height** of the consensus algorithm is identified by an
integer, represented by the `h_p` variable in the pseudo-code.
The height `h_p` of consensus is concluded when the process reaches a decision
on a value `v`, represented in the pseudo-code by the action
`decision_p[h_p] = v` (line 51).
A this point, the process increases `h_p` (line 52) and starts the next height
of consensus, in which the same algorithm is executed again.

For the sake of the operation of the consensus algorithm, heights are
completely independent executions. For this reason, in this specification we
consider and discuss the execution of a **single height of consensus**.

### Rounds

A height of consensus is organized into rounds, identified by integers and
always starting from round 0.
The round at which a process `p` is identified in the
[pseudo-code][pseudo-code] by the `round_p` variable.
A successful round of consensus leads the process to decide on the value `v`
proposed in that round, as in the pseudo-code block from line 49.
An unsuccessful round of consensus does not decide a value and leads the
process to move to the next round, as in the pseudo-code block from line 65,
or to skip to an arbitrary higher round, as in the block from line 55.

The execution of each round of consensus is led by a process selected as the
**proposer** of that round.
Tendermint assumes the existence of a deterministic proposer selection
algorithm represented in the pseudo-code by calls to the `proposer(h, r)`
external function that returns the process that should lead round `r` of
consensus height `h`.

### Round Steps

A round of consensus is organized into a sequence of three round steps:
`propose`, `prevote`, and `precommit`, as defined in line 4 of the
[pseudo-code][pseudo-code].
The current round step of a process `p` is stored in the `step_p` variable.
In general terms, a process performs one or more **actions** when entering or
moving into a new round step.
Plus, the reception a given set of **events** while in a round step, leads the
process to move to the successive round step.

#### Propose

The `propose` round step is the first step of each round.
In fact, a process `p` sets its `step_p` to `propose` as part of the execution
of the `StartRound(round)` function, where it also increases `round_p` to the
new round `round`.
The `propose` step is the only round step that is asymmetric, meaning that
different processes perform different actions when starting it.
More specifically, the round's proposer has a distinguish role in this round step.

In the `propose` round step, the **proposer** of the current round selects the
value to be the proposed in that round and **broadcast**s the proposed value to all
processes (line 19).
All other processes start a **timeout** (line 21) to limit the amount of time
they will spend in the `propose` step while waiting for the value send by the
proposer.

#### Prevote

The `prevote` round step has the role to validate the value proposed in the
`propose` step.
The value proposed by round's proposer can be accepted (lines 24 or 30) or
rejected (lines 26 or 32) by the process.
A value can be also rejected if not received from the proposer when the timeout
scheduled in the `propose` step expires (line 59).

The action taken by a process when it moves from the `propose` to the `prevote`
step is to **broadcast** a message to inform all processes whether it has accepted
or not the proposed value.
The remaining of this step consists of collecting the messages that other
processes have broadcast in the same round step.
In the case where there is no agreement on whether the value proposed on the
current round is acceptable or not, the process schedules a **timeout** (line
35) to limit the amount of time it waits for an agreement on the validity or
not of the proposed value.

#### Precommit

The `precommit` round step is when it is defined whether a round of consensus
has succeeded or not.
In the case of a successful round, the decision value has been established and
it is committed: the consensus height is done (line 51).
Otherwise, the processes will need an additional round to attempt reaching a
decision (line 67).

The action taken by a process when it moves from the `prevote` step to the
`precommit` step is to **broadcast** a message to inform whether an agreement
has been observed in the `prevote` round step (lines 40, 45, or 63).
The remaining of this step consists of collecting the messages that other
processes have broadcast in the same round step.
If there is conflicting information on the received messages, the process
schedules a **timeout** (line 48) to limit the amount of time it waits for the
round to succeed; if this timeout expires, the round has failed.

**Important**: contrarily to what happens in previous round steps, the actions
that are associated to the `precommit` round step do not require the process to
actually be in the `precommit` round step. More specifically:

- If a process is at any round step of round `round_p` and the conditions from
  line 47 of the pseudo-code are observed, the process will schedule a timeout
  for the `precommit` round step (line 48);
- If the timeout for the `precommit` round step expires, line 65 of the
  pseud-code is executed. If the process is still on the same round when it was
  scheduled, the round fails and a new round is started (line 67);
- If a process observes the conditions from line 49 of the pseudo-code for
  **any round** `r` of its current height `h_p`, the decision value is
  committed and the height of consensus is done.
  Notice that `r` can be the current round (`r = round_p`), a previous failed
  round (`r < round_p`), or even a future round (`r > round_p`).

> Those special conditions are currently listed and discussed in the
> [Exit transitions](../english/consensus/README.md#exit-transitions) section
> of the Malachite specification.

## Messages

The Tendermint consensus algorithm defines three message types, each type
associated to a [round step](#round-steps):

- `⟨PROPOSAL, h, r, v, vr⟩`: broadcast by the process returned by `proposer(h, r)`
  function when entering the [`propose` step](#propose) of round `h` of height `h`.
  Carries the proposed value `v` for height `h` of consensus.
  Since only proposed values can be decided, the success of round `r` depends
  on the reception of this message.
- `⟨PREVOTE, h, r, *⟩` broadcast by all processes when entering the
  [`prevote` step](#prevote) of round `h` of height `h`.
  The last field can be either the unique identifier `id(v)` of the value
  carried by a `⟨PROPOSAL, h, r, v, *⟩` message, meaning that it was received
  and `v` has been accepted, or the special `nil` value otherwise.
- `⟨PRECOMMIT, h, r, *⟩`: broadcast by all processes when entering the
  [`precommit` step](#precommit) of round `h` of height `h`.
  The last field can be either the unique identifier `id(v)` of a proposed
  value `v` for which the process has received an enough number of
  `⟨PREVOTE, h, r, id(v)⟩` messages, or the special `nil` value otherwise.

Before discussing in detail the role of each message in the protocol, it is
worth highlighting the main aspects that differentiate the adopted messages.
The `PROPOSAL` message is assumed to carry the proposed value, which may have
an arbitrary size; we refer to it as the "full" value `v`.
The propagation of large values, included in `PROPOSAL` messages, in practice
requires specific and efficient data dissemination protocols.
Implementations typically split the `PROPOSAL` message into multiple parts,
independently propagated and reconstructed at the receiver side.
The `PREVOTE` and `PRECOMMIT` messages are generally called [votes](#votes).
They typically have a fixed size and are expected to be much smaller than
`PROPOSAL` messages.
The main reason for that is that they do not carry a "full" value `v`, but
instead an unique identified `id(v)` of a proposed value `v` carried by an
associated `PROPOSAL` message.

### Proposals

Proposals are produced and broadcast by the `StartRound(round)` function of the
[pseudo-code][pseudo-code], by the process selected returned by the
`proposer(h_p, round)` external function, where `round = round_p` is the
started round.

Every process expects to receive the `⟨PROPOSAL, h, r, v, *⟩` broadcast by
`proposer(h, r)`, as its reception is a condition for all state transitions
that propitiate a successful round `r`, namely the pseudo-code blocks starting
from lines 22 or 28, 36, and 49.
The success of round `r` results in `v` being the decision value for height `h`.

#### Value Selection

The proposer of a round `r` defines which value `v` it will propose based on
the values of the two state variables `validValue_p` and `validRound_p`.
They are initialized to `nil` and `-1` at the beginning of each height, meaning
that the process is not aware of any proposed value that has became **valid**
in a previous round.
A value becomes **valid** when a `PROPOSAL` for it and an enough number of
`PREVOTE`s accepting it are received during a round.
This logic is part of the pseudo-code block from line 36, where `validValue_p`
and `validRound_p` are updated.

If the proposer `p` of a round `r` of height `h` has `validValue_p != nil`,
meaning that `p` knows a valid value, it must propose that value again.
The message it broadcasts when entering the `prevote` step of round `r` is
thus `⟨PROPOSAL, h, r, validValue_p, validRound_p⟩`.
Note that, by construction, `r < validRound_p < -1`.

If the proposer `p` of a round `r` of height `h` has `validValue_p = nil`, `p`
may propose any value it wants.
The external function `getValue()` is invoked, which returns a new value to be
proposed.
The message it broadcasts when entering the `prevote` step of round `r` is
thus `⟨PROPOSAL, h, r, getValue(), -1⟩`.
Observe that this is always the case in the first round `r = 0` of any height
`h`, and the most common case in ordinary executions.

#### Byzantine Proposers

A correct process `p` will only broadcast a `⟨PROPOSAL, h, r, v, vr⟩` message
if `p = proposer(h, r)`, i.e., it is the round's proposer, it will follow the
value selection algorithm and propose at most one value `v` per round.

A Byzantine process `q` may not follow any of the above mentioned algorithm
rules. More precisely:

1. `q` may broadcast a `⟨PROPOSAL, h, r, v, vr⟩` message while `q !=  proposer(h, r)`;
2. `q` may broadcast a `⟨PROPOSAL, h, r, v, -1⟩` message while `v != validValue_q != nil`;
3. `q` may broadcast a `⟨PROPOSAL, h, r, v, vr⟩` message while `-1 < vr != validRound_q`;
4. `q` may broadcast multiple `⟨PROPOSAL, h, r, *, *⟩` messages, each proposing a different value.

Attack 1. is simple to identify and deal as long as proposals include the
**digital signature** of their senders, given that the
[proposers](#proposer-selection) for any given round of a height are assumed to
be a priori known by all participants.

Attacks 2. and 3. are constitute forms of the **amnesia attack** and are harder
to identify.
Notice, however, that a correct process checks whether it can accept a proposed
value `v` with valid round `vr` based in the content of its state variables
`lockedValue_p` and `lockedRound_p` (lines 23 and 29) and are likely to reject
such proposals.

Attack 4. constitutes a double-signing or **equivocation** attack.
It is virtually impossible to prevent, and the only approach for a correct
process is to only consider the first `⟨PROPOSAL, h, r, v, *⟩` received in the
`propose` step, which can be accepted or rejected.
However, it is possible that a different `⟨PROPOSAL, h, r, v', *⟩` with
`v' != v` is received and triggers the state transitions from the `prevote` or
`precommit` round steps.
So, a priori, a correct process must potentially store all the  multiple
proposals broadcast by a Byzantine proposer.

> TODO: storing all received proposals, from a Byzantine proposer, constitutes
> an attack vector.
> Previous content:
>  - A correct process could in theory only consider the first proposal
>    message received for a round, say it proposes `v`.
>    The problem of this approach is that `2f + 1` processes might accept, or
>    even decide, a different value `v' != v`.
>    By ignoring the equivocating proposal for `v'`, the process will not be
>    able to vote for or decide `v'`, which in Tendermint consensus algorithm
>    may compromise liveness.
>
>    **Note:** in contrast to algorithms from theoretical papers, a node running Tendermint consensus terminates
>    a consensus instance after it has decided; it will no longer react on messages from that instance or send
>    messages for that instance (if it is a process). In contrast, in theoretical algorithms, even after deciding, processes keep on
>    participating and sending messages. In the theoretical setting these processes will help the process that
>    has only considered to first proposal from a faulty proposer, to make progress. In Tendermint consensus, this
>    help is not there. Thus, there is above discussed liveness issue.
>  - Storing multiple proposal messages for the same round is, by itself, an
>    attack vector. Validators must thus restrict the number of proposal
>    messages stored in rounds where multiple proposals are produced.

Notice that while hard to prevent, equivocation attacks are easy to detect,
once distinct messages for the same height, round, and round step are received
and they are signed by the same process.
For a more comprehensive discussion on misbehavior detection, evidence
production and dissemination refer to this [document](./misbehavior.md).

### Votes

Vote is the generic name for `⟨PREVOTE, h, r, *⟩` and `⟨PRECOMMIT, h, r, *⟩` messages.
Tendermint includes two voting steps, the `prevote` and the `precommit` round
steps, where the corresponding votes are exchanged.

Differently from proposals, that are broadcast by the rounds' proposers to all
processes (1-to-n communication pattern), every process is expected to
broadcast its votes (n-to-n communication pattern), two votes per round.
However, while proposals carry a (full) proposed value `v`, with variable size,
votes only carry a (fixed-size and small) unique identifier `id(v)` of the
proposed value, or the special value `nil` (which means "no value").

Moreover, the analysis of the [pseudo-code][pseudo-code] reveals that, while
the reception of a proposal is considered by itself an event that may trigger a
state transition, the reception of a _single_ vote message does not by itself
trigger any state transition.
The main reason for that is the fact that up to `f` processes are assumed to be
Byzantine, which by definition can produce arbitrary vote messages.
As a result, no information produced by a single, or by a set with at most `f`
processes can be considered legit and should not drive the operation of correct
processes.

#### Voting power

Up to this point, this document is aligned with the pseudo-code and has the
following failure assumptions:

1. The algorithm tolerates `f` Byzantine-faulty processes, which may behave
   arbitrarily;
2. The algorithm requires that less than one third of the processes are
   Byzantine. So, if `n` is the total number of processes, the algorithm
   assumes `f < n/3`. In fact, the algorithm considers a set of `n = 3f + 1`
   processes.

These are built from the common assumption that processes are homogeneous, in
the sense that the vote of any process counts the same: one process, one vote.
In other words, all processes have the same voting power.

Tendermint was designed to support the operation of blockchains that adopt the
Proof-of-Stake (PoS) strategy.
In this strategy, processes are assumed to stake (deposit) some amount to be
active actors in the blockchain and to have a voting power that is proportional
to the staked amount.
In other words, when adopting the PoS framework, processes are assumed to have
distinct voting powers.
The failures assumptions are thus updated as follows:

1. Each process `p` owns or has an associated voting power `p.power > 0`;
2. The system is composed by a set of process whose aggregated or total voting
   power is `n`;
3. The maximum voting power owned by or associated to Byzantine validators is
   assumed to be `f < n/3`.

> The staking is typically managed at the application level and Tendermint
> is informed or configured about the next validator set.
> This is how the process works in Cosmos, where Tendermint and application
> interacts via ABCI (application-blockchain interface), a standard,
> language-agnostic communication protocol.

This means, in particular, that when `f + 1` is used in the pseudo-code, it
must be considered a set of processes whose aggregated voting power is strictly
higher than `f`, namely strictly higher than `1/3` of the processes' total
voting power `n`.
This means that, among the considered processes, **at least one process is correct**.

Analogously, when `2f + 1` is used in the pseudo-code, this should be interpreted
as a set of processes in which the aggregated voting power of correct processes
in the set is strictly higher than the aggregated voting power of (potentially)
Byzantine processes in the set.
In other words, **the majority of the processes is correct**.

#### Byzantine Voters

A correct process `p` will only broadcast one `⟨PREVOTE, h, r, *⟩` and one
`⟨PRECOMMIT, h, r, *⟩` messages in round `r` of height `h`.
The votes `p` broadcasts in each round step will carry either the unique
identifier `id(v)` of the value `v`, received in a `⟨PROPOSAL, h, r, v, *⟩`
message from `proposer(h, r)`, or the special value `nil`.

Byzantine processes, however, can broadcast multiple vote messages for the same
round step, carrying any value they received or produced, including values that
were not proposed on any round, and the special `nil` value.
The main attacks that are worth considering, because of their potential of
inducing undesirable behaviour, are two:

1. **Equivocation**: a Byzantine process can broadcast multiple
   `⟨PREVOTE, h, r, *⟩` or `⟨PRECOMMIT, h, r, *⟩` messages in the same round
   `r` of height `h`, for distinct values: `nil`, `id(v)`, or `id(v')`
   with `v != v'`.
2. **Amnesia**: a Byzantine process `q` can broadcast `⟨PREVOTE, h, r, *⟩` or
   `⟨PRECOMMIT, h, r, *⟩` messages for values that are not in line with the
   expected contents of its `lockedValue_q` and `lockedRound_q` variables.

Since Byzantine processes can always produce **equivocation attacks**, a way
that a correct process can deal with them is by only considering the first
`⟨PREVOTE, h, r, *⟩` or `⟨PRECOMMIT, h, r, *⟩` messages received from a process
in a round `r` of height `h`.
Different (equivocating) versions of the same message from the same sender
should, from a defensive point of view, be disregarded and dropped by the
consensus logic as they were duplicated messages.
The reason for which is the fact that a Byzantine process can produce an
arbitrary number of such messages.

> For a more comprehensive discussion on producing evidences of equivocation
> refer to this [document](./misbehavior.md).

Unfortunately, there are multiple scenarios in which correct processes may
receive equivocating messages from Byzantine voters in different orders, and
by only considering the first received one, they may end up performing
different state transitions in the consensus protocol.
While this does not pose a threat to the safety of consensus, this might
produce liveness issues, as correct processes may be left behind in the
consensus computation.

> TODO: more details on this [here](somewhere).
> Previous content:
>  - A correct validator could "in theory" only consider the first vote message
>    received from a sender per round step, say it carries `id(v)`.
>    The problem of this approach is that `2f + 1`  validators might only
>    consider a different vote message from the same sender and round step,
>    carrying `id(v')` with `v' != v`. This may lead other validators to decide `v'`.
>    By ignoring the equivocating voting message carrying `id(v')`, the
>    validator might not be able to decide `v'`, which may compromise
>    liveness of the consensus algorithm.

The **amnesia attack** is also virtually impossible to prevent and it is also
harder to detect than equivocation ones.
A correct process `p` that broadcasts a `⟨PRECOMMIT, h, r, id(v)⟩` must update
its variables `lockedValue_p ← v` and `lockedRound_p ← r`, as shown in the
pseudo-code block from line 36.
From this point, `p` is locked on value `v`, which means that upon receiving a
`⟨PROPOSAL, h, r', v', vr⟩` message for a round `r' > r`, it must reject the
proposed value `v'` if it does not match its locked value `v`, i.e., it must
broadcast a `⟨PREVOTE, h, r', nil⟩` message.
The only possible exception is when the proposal's valid round `vr > r`
corresponds to a round where there was an agreement on the proposed value
`v' != lockedValue_p`, as shown in the pseudo-code block from line 28.
In this case, issuing a `⟨PREVOTE, h, r', id(v')⟩` can be justified as a
correct behaviour provided that `p` is able to prove the existence of a
`2f + 1 ⟨PREVOTE, h, vr, id(v')⟩` set of messages accepting `v'`.

> A variation of Tendermint consensus protocol, known as
> [Accountable Tendermint][accountable-tendermint], proposes some changes in
> the algorithm to render it possible to detect and produce evidence for the
> amnesia attack (see also [#398](https://github.com/informalsystems/malachite/issues/398)).

## External Components

The [pseudo-code][pseudo-code] of the consensus algorithm includes calls to
functions and primitives that are not defined in the pseudo-code itself, but
are assumed to be implemented by the processes running the consensus protocol.

### Functions

#### Proposer Selection

The first external function is `proposer(h, r)` that returns the process
selected as the proposer of round `r` of height `h` of consensus. The roles of
the proposer of a round are described in the [propose round step](#propose).

> The formalization of the properties requires for the proposer selection
> algorithm is a work in progress, see
> https://github.com/informalsystems/malachite/issues/396.

#### Proposal value

The external function `getValue()` is invoked by the proposer of a round as
part of the transition to the [propose round step](#propose).
It should return a value to be proposed by the process `p` for the current
height of consensus `h_p`.

> TODO: synchronous/asynchronous implementations, currently discussed
> [here](../english/consensus/README.md#asynchronous-getvalue-and-proposevaluev).

#### Validation

The external function `valid(v)` is invoked by a process when it receives a
`⟨PROPOSAL, h, r, v, *⟩` from the proposer of round `r` of height `h`.
It should return whether the `v` is a valid value according to the semantics of 
the "client" of the consensus protocol, i.e., the application that uses
consensus to agree on proposed values.

> TODO: relevant observation:
> - Validation typically depends on `h` as well, in particular on the
>   application state at blockchain height `h`. It should not ordinarily
>   depend on `r`, since the application state should not change over rounds.
> - Determinism: is `valid(v)` a function?
> - Needed because of validity property of consensus, defined in the paper

### Primitives

#### Network

The only network primitive adopted in the pseudo-code is the `broadcast`
primitive, which should send a given [consensus message](#messages) to all
processes, thus implementing a 1-to-n communication primitive.

> TODO: reliable broadcast properties needed for consensus messages, and the
> more comprehensive and strong properties required for certificates (sets of
> 2f + 1 identical votes), and certified proposals.

#### Timeouts

The `schedule` primitive is adopted in the pseudo-code to schedule the
execution of `OnTimeout<Step>(height, round)` functions, where `<Step>` is one
of `Propose`, `Prevote`, and `Precommit` (i.e., the three [round steps](#round-steps)),
to the current time plus the duration returned by the corresponding functions
`timeout<Step>(round)`.

> TODO: assumptions regarding timeouts, they should increase over time, GST, etc.

> TODO: most timeouts can be cancelled when the associated conditions are not
> any longer observed (round or height changed, round step changed).

[^1]: This document adopts _process_ to refer to the active participants of the
  consensus algorithm, which can propose and vote for values. In the blockchain
  terminology, a _process_ would be a _validator_. In the specification both
  names are adopted and are equivalent.

[pseudo-code]: ./pseudo-code.md
[accountable-tendermint]: ./misbehavior.md#misbehavior-detection-and-verification-in-accountable-tendermint
