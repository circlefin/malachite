# Consensus Algorithm

Malachite adopts the Tendermint consensus algorithm from the paper
["The latest gossip on BFT consensus"](https://arxiv.org/abs/1807.04938)
([PDF](https://arxiv.org/pdf/1807.04938)), by Ethan Buchman, Jae Kwon,
and Zarko Milosevic, last revised in November 2019.

The **pseudo-code** of the algorithm, referenced several times in this
specification, is the Algorithm in page 6, that for simplicity and easy
reference is copied into the [pseudo-code.md][pseudo-code] file.

## Overview

A consensus algorithm is run by a (previously defined) set of processes, some
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

> Byzantine nodes are assumed to not to be able to break digital signatures,
> that is, pretend to forward messages by correct nodes that were never send
> (a.k.a. non-masquerading).
>
> FIXME: move this to the communication assumptions?

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
external function that returns the process that should led round `r` of
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
> of the specification.

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

Attack 1. is simple to identify and deal as long as proposals contain **digital signatures**.

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
> an attack vector

Notice that while hard to prevent, equivocation attacks are easy to detect,
once distinct messages for the same height, round, and round step are received
and they are signed by the same process.

> TODO: reference to evidence production.

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

#### Byzantine Voters

TODO:

> TODO: storing all received votes, from a Byzantine equivocating voter,
> constitutes an attack vector

## External Functions

TODO: Describe the functions used in the pseudo-code.

This includes `proposer()`, `valid()`, `getValue()`.

Possibly **broadcast** and **schedule** as well.

[pseudo-code]: ./pseudo-code.md
