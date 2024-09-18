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
> of the specification.

## Messages

TODO: there is a lot of content to move or re-use [here](../english/consensus/README.md#message-handling)

## Events

TODO: describe the "complex" events derived from the reception of several
single events, the ones we deliver to the consensus state-machine.

## External components

TODO: Describe the functions used in the pseudo-code.

This includes `proposer()`, `valid()`, `getValue()`.

Possibly **broadcast** and **schedule** as well.

[pseudo-code]: ./pseudo-code.md
