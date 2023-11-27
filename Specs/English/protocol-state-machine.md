# Consensus protocol: state-machine

The consensus protocol consists of a sequence of **heights**,
identified by natural numbers `h`.
We define a [`Height(h)` state machine](#height-state-machine) to represent the
operation of a height `h`.

Each height of consensus consists of a number of **rounds**,
identified by natural numbers `r`.
We define a [`Round(r)` state machine](#round-state-machine) to represent the
operation of each of the rounds `r` of a height.

## Height state machine

The state machine `Height(h)` represents the operation of a height `h` of consensus.

The considered set of states are:

- Unstarted
  - Initial state
  - Can be used to store messages for unstarted height `h`
  - In the algorithm when `hp < h`, where `hp` is the node's current round, and `decisionp[h] == nil`
- InProgress: 
  - Actual consensus execution
  - In the algorithm when `hp == h` and `decisionp[h] == nil`
  - Controls the operation of multiple round state machines `Round(r)`, where `r` is a natural number
- Decided
  - Final state
  - May also include the commit/execution of decided value
  - In the algorithm when `hp >= h` and `decisionp[h] != nil`

The table below summarizes the major state transitions in the `Height(h)` state machine.
The `Ref` column refers to the line of the pseudo-code where the events can be found.

| From | To | Event | Action | Ref |
|-------|-----------|-----------|--------|-----|
| Unstarted | InProgress | `start_height` | send `start` to `Round(0)` | L10, L54 |
| Round(r) | Decided | `decide(r', v)` | emit `decide(h, v)` <br> stop to every `Round(r)` | L49 |
| Round(r) | Round(r+1) | `next_round(r+1)` | send `start` to `Round(r+1)` | L65 |
| Round(r) | Round(r') | `next_round(r')` | send `start` to `Round(r')` | L55 |

A height `h` consists of multiple rounds of consensus, always starting from
round `0`.
The `Unstarted` state is intended to store events and messages regarding height `h`
before its execution is actually started.

The height is concluded when a decision is reached in _any_ of its rounds.
The `Decided` state is intended to represent that a decision has been reached,
while it also allows storing the summary of a decided height.

Once the node moves to the `Decided` state of a height, the operation of
_every_ round `Round(r)` should be concluded.
The representation of this transition needs to be improved, for now it is
considered that the corresponding `Round(r)` state machines are killed.

<!---

### InProgress height

The table below represents transitions within the `InProgress` state,
representing the events that lead a node to start new round of consensus:

Each round `r` of consensus is represented by a state machine `Round(r)`.
There is a single round _in progress_ at a time, which is always the last
`Round(r)` state machine to receive the `start` command.

A round `r` may not succeed on reaching a decision.
In this case, the successive round `r+1` is started.
The uncessful `Round(r)` state machine is not killed at this point, as messages
referring to that round can still be required for the operation of future rounds.

While in a round `r`, a node may realize that several nodes are already in a
future round `r' > r`.
When this happens, the node switches to round `r'`, skipping both the current
and the possible intermediate rounds.

--->

## Round state-machine

The state machine `Round(r)` represents the operation of a round `r` of consensus.
It is controlled (started and stopped) by an instance of the `Height(h)` state machine.

The considered set of states are:

- Unstarted
  - Initial state
  - Can be used to store messages early received for this round
  - In the algorithm when `roundp < r`, where `roundp` is the node's current round
- InProgress
  - Actual consensus single-round execution
  - In the algorithm when `roundp == r`
- Stalled
  - Final state for an unsuccessful round
  - In the algorithm when `roundp > r`
  - Consists of the substates: `propose`, `prevote`, and `precommit`
- Decided
  - Final state for an successful round

The table below summarizes the major state transitions in the `Round(r)` state machine.
The `Ref` column refers to the line of the pseudo-code where the events can be found.

| From       | To         | Ev Name           | Event  Details                                                      | Action                    | Ref |
| ------------ | ------------ | ------------------- | --------------------------------------------------------------------- | --------------------------- | ----- |
| InProgress | InProgress | PrecommitAny      | `2f + 1 ⟨PRECOMMIT, h, r, *⟩` <br> for the first time             | schedule `TimeoutPrecommit(h, r)` | L47 |
| InProgress | Unstarted (?)  | TimeoutPrecommit  | `TimeoutPrecommit(h, r)`                                            | `next_round(r+1)`         | L65 |
| InProgress | Unstarted (?) | RoundSkip(r')     | `f + 1 ⟨*, h, r', *, *⟩` with `r' > r`                      | `next_round(r')`          | L55 |
| InProgress | Decided    | PrecommitValue(v) | `⟨PROPOSAL, h, r, v, *⟩` <br> `2f + 1 ⟨PRECOMMIT, h, r, id(v)⟩` | `commit(v)`               | L49 |


<!--

The following two state transitions are associated with the round-skipping mechanism.
**TODO:** They need to be reviewed.

| From | To | Event | Action | Ref |
|-------|-----------|-----------|--------|-----|
| InProgress | Stalled | `f + 1 ⟨PREVOTE, h, r', *, *⟩` with `r' > r` | emit `next_round(r')` | L55 |
| InProgress | Stalled | `f + 1 ⟨PRECOMMIT, h, r', *, *⟩` with `r' > r` | emit `next_round(r')` | L55 |

> There is an open question in this specification related to the round-skipping
> state transitions, as they are the only to have as input messages from a round
> `r'` that is not the state machine round `r`.
> It would be possible to have these events processed by the `Round(r')` state
> machine, instead, as this is the round to which the messages belong.
> In this case, if the `Round(r')` state machine is on the `Unstarted` state and
> the events are observed, the round skip event `next_round(r')` could be produced.
> The `Round(r)` state machine, in this case, could process this event instead,
> moving to the `Stalled` state in the same way as it is now.

-->

### InProgress round

The table below summarizes the state transitions within the `InProgress` state
of the `Round(r)` state machine.
There can only be a single round state machine at the `InProgress` state at a
time, which represents the node's current round of consensus.
The following state transitions therefore represent the core of the consensus algorithm.
The `Ref` column refers to the line of the pseudo-code where the events can be found.

| From      | To        | Ev Name          | Ev Details                                                                                  | Actions and Return                                                                        | Ref |
| ----------- | ----------- | ------------------ | --------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- | ----- |
| Unstarted | propose   | StartProposer(v) | `start` with `proposer(h, r) = p` and `v = getValue()`                                                  | broadcast `⟨PROPOSAL, h, r, v, validRound⟩`                                                  | L19 |
| Unstarted | propose   | StartNonProposer | `start` with `proposer(h, r) != p` (optional restriction)                                   | schedule `TimeoutPropose(h, r)`                                                                   | L21 |
| propose   | prevote   | Proposal(v, -1)  | `⟨PROPOSAL, h, r, v, −1⟩`                                                                | broadcast `⟨PREVOTE, h, r, {id(v), nil}⟩`                                                            | L22 |
| propose   | prevote   | Proposal(v, vr)  | `⟨PROPOSAL, h, r, v, vr⟩` <br> `2f + 1 ⟨PREVOTE, h, vr, id(v)⟩` with `vr < r`           | broadcast `⟨PREVOTE, h, r, {id(v), nil}⟩`                                                            | L28 |
| propose   | prevote   | TimeoutPropose   | `TimeoutPropose(h, r)`                                                                      | broadcast `⟨PREVOTE, h, r, nil⟩`                                                                     | L57 |
| prevote   | prevote   | PolkaAny         | `2f + 1 ⟨PREVOTE, h, r, *⟩` <br> for the first time                                       | schedule `TimeoutPrevote(h, r)⟩`                                                                 | L34 |
| prevote   | precommit | PolkaValue(v)    | `⟨PROPOSAL, h, r, v, ∗⟩` <br> `2f + 1 ⟨PREVOTE, h, r, id(v)⟩` <br> for the first time  | update `lockedValue, lockedRound, validValue, validRound`,<br /> broadcast `⟨PRECOMMIT, h, r, id(v)⟩` | L36 |
| prevote   | precommit | PolkaNil         | `2f + 1 ⟨PREVOTE, h, r, nil⟩`                                                             | broadcast `⟨PRECOMMIT, h, r, nil⟩`                                                                    | L44 |
| prevote   | precommit | TimeoutPrevote   | `TimeoutPrevote(h, r)`                                                                      | broadcast `⟨PRECOMMIT, h, r, nil⟩`                                                                   | L61 |
| precommit | precommit | PolkaValue(v)    | `⟨PROPOSAL, h, r, v, ∗⟩` <br>  `2f + 1 ⟨PREVOTE, h, r, id(v)⟩` <br> for the first time | update `validValue, validRound`                                                            | L36 |

The ordinary operation of a round of consensus consists on the sequence of
round steps `propose`, `prevote`, and `precommit`, represented in the table.
The conditions for concluding a round of consensus, therefore for leaving the
`InProgress` state, are presented in the next sub-section.

All the state transitions represented in the table on consider message and
events referring to the node's current round `r`.
In the pseudo-code this current round of a node is referred as `round_p`.

There is, however, an exception: the transition `L28` requires the node to have
access to `PREVOTE` messages from a previous round `r' < r`.
Ideally, messages for each round `r` should be handled by the corresponding
`Round(r)` state machine.
This transition constitutes an exception that have to be handled in a proper way.

<!---

### Exit transitions

The table below summarizes the state transitions within the major states of `Round(r)`.
The transactions from state `InProgress` consider that node can be at any of
its substates, whose transitions were covered in the previous section.
The `Ref` column refers to the line of the pseudo-code where the events can be found.

| From       | To         | Ev Name           | Event  Details                                                      | Action                    | Ref |
| ------------ | ------------ | ------------------- | --------------------------------------------------------------------- | --------------------------- | ----- |
| InProgress | InProgress | PrecommitAny      | `2f + 1 ⟨PRECOMMIT, h, r, *⟩` <br> for the first time             | `timeout_precommit(h, r)` | L47 |
| InProgress | Unstarted  | TimeoutPrecommit  | `TimeoutPrecommit(h, r)`                                            | `next_round(r+1)`         | L65 |
| InProgress | Unstarted  | RoundSkip(r')     | `f + 1 ⟨PREVOTE, h, r', *, *⟩` with `r' > r`                      | `next_round(r')`          | L55 |
| InProgress | Decided    | PrecommitValue(v) | `⟨PROPOSAL, h, r, v, *⟩` <br> `2f + 1 ⟨PRECOMMIT, h, r, id(v)⟩` | `commit(v)`               | L49 |
|            |            |                   |                                                                     |                           |     |

The first two transitions are associated to unsuccessful rounds.
To leave an unsuccessful round, a node has to schedule a `TimeoutPrecommit`
timeout, which expiration leads it to the subsequent round.
Once the next round `r+1` is started, by the `Height(h)` state machine, the
`Round(r)` state machine moves to the `Stalled` state.
The only possible transition from this point is upon the decision of a value to
the `Decided` final state.

The next two transitions are associated with the round-skipping mechanism.
Once the `Height(h)` moves the process to the future round `r'`, the `Round(r)`
state machine moves to the `Stalled` state.
The only possible transition from this point is upon the decision of a value to
the `Decided` final state.

The last two transitions are associated with the decision of a value in round `r`.
It might occur while this is the current round (`InProgress` state) or after it
was concluded without success (`Stalled` state).

--->

## Events

Description of the events considered by the algorithm.

We should consider renaming them for more clarity.
The production of such events requires, in most cases, the definition of a
state machine to produce them.

### `⟨PROPOSAL, h, r, v, *⟩`

A `PROPOSAL` message for round `(h,r)`.
Must be received from (i.e., signed by) `proposer(h, r)`.

The algorithm considers that this message carries the (full) value `v`.
This specification should consider that the carried value can be obtained in a
different way.
This event, in this case, consists of the combination of possible multiple
message or events that have as a result the production of this event for the
algorithm.

### `2f + 1 ⟨PREVOTE, h, r, *⟩`

Quorum of `PREVOTE` messages for a round `(h, r)`.

The last field value can be:

- `id(v)`: quorum of votes for a value
- `nil`: quorum of votes for nil
- `any`: quorum of votes for multiple values and/or nil

### `2f + 1 ⟨PRECOMMIT, h, r, *⟩`

Quorum of `PRECOMMIT` messages for a round `(h, r)`.

The last field value can be:

- `id(v)`: quorum of votes for a value
- `nil`: quorum of votes for nil
- `any`: quorum of votes for multiple values and/or nil

### `Timeout*(h, r)`

Timeout events.
They must be scheduled in order to be triggered.

### `f + 1 ⟨*, h, r, *⟩`

For round-skipping, needs to be properly evaluated.

## Pending of description

- [ ] Events considered by every state machine
  - Events produced by one state machine and processed by another
  - External events, produced by the environment (e.g. messages and timeouts)
- [ ] State machine producing complex events (e.g. `2f + 1` message X)
- [ ] Routing of events from the higher-level state machine to lower-level state machines

