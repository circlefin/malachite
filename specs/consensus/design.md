# Malachite Consensus Design

[Tendermint consensus algorithm](./overview.md) works by a set of processes
exchanging messages over a network, and the local consensus instances act on
the incoming messages if certain conditions are met (e.g., if a threshold
number of specific messages is received a state transition should happen).

The architecture of the Tendermint consensus implementation in Malachite separates:

- counting messages in a **vote keeper** ([Quint][quint-votekeeper]),
- creating consensus inputs in a **driver** ([Quint][quint-driver]),
  e.g., if a threshold is reached
- doing the state transition depending on the consensus input in the
  **state machine** ([Quint][quint-sm])

A detailed executable specification of these functionalities are given in
[Quint][quint-spec].
In this (English) document we discuss the main underlying principles, focusing
on the operation of the consensus **state machine**.

In a separate [message handling](./message-handling.md) document we discuss how
the implementation treats incoming messages, which messages to store, and on
what conditions to generate consensus inputs.

## Round state machine

The consensus state-machine operates on complex **events** that reflect the
reception of one or multiple **messages**, combined with state elements and the
interaction with other modules.

The state machine represents the operation of consensus at a single `Height(h)` and `Round(r)`.
The diagram below offers a visual representation of the state machine. It shows the input events, using green for simple inputs (e.g. timeouts, proposal)
and red for the complex events (e.g. `ProposalAndPolkaCurrent` is sent to the state machine when a valid proposal and a polka of prevotes have been received).
The actions are shown in italics (blue) and the output messages are shown in blue.

![Consensus SM Diagram](./assets/sm_diagram.jpeg)

The set of states can be summarized as:

- `Unstarted`
  - Initial state
  - Can be used to store messages early received for this round
  - In the algorithm when `round_p < r`, where `round_p` is the process' current round
- InProgress (`Propose`, `Prevote`, `Precommit`)
  - Actual consensus single-round execution
  - In the algorithm when `round_p == r`
- `Commit`
  - Final state for a successful round

### Exit transitions

The table below summarizes the major state transitions in the `Round(r)` state machine.
The transactions from state `InProgress` consider that process can be at any of
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

### Validity Checks

The pseudocode of the algorithm includes validity checks for the messages. These checks have been moved out of the state machine and are now performed by the `driver` module.
For this reason:
- `L22` is covered by `Proposal(v, -1)` and `InvalidProposal(v, -1)`
- `L28` is covered by `ProposalAndPolkaPrevious(v, vr)` and `InvalidProposalAndPolkaPrevious(v, vr)`
- `L36` and `L49` are only called with valid proposal

TODO - show the full algorithm with all the changes

### Asynchronous getValue() and ProposeValue(v)

The original algorithm is modified to allow for asynchronous `getValue()`. The details are described below.

<table>
<tr>
<th>arXiv paper</th>
<th>Async getValue()</th>
</tr>

<tr >
<td>

```go
11: Function StartRound(round):
12:    round_p ← round
13:    step_p ← propose
14:    if proposer(h_p, round_p) = p then
15:       if validValue_p != nil then
16:          proposal ← validValue_p


17:       else
18:          proposal ← getValue()

19:       broadcast ⟨PROPOSAL, h_p, round_p, proposal, validRound_p⟩
20:    else
21:       schedule OnTimeoutPropose(h_p, round_p) 
             to be executed after timeoutPropose(round_p)
```

</td>

<td>

```go
11: Function StartRound(round):
12:    round_p ← round
13:    step_p ← propose
14:    if proposer(h_p, round_p) = p then
15:       if validValue_p != nil then
16:          proposal ← validValue_p

             broadcast ⟨PROPOSAL, h_p, round_p, proposal, validRound_p⟩
17:       else
             getValue() // async
             schedule OnTimeoutPropose(h_p, round_p) 
                to be executed after timeoutPropose(round_p)
20:    else
21:       schedule OnTimeoutPropose(h_p, round_p) 
             to be executed after timeoutPropose(round_p)
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

```go
68: upon PROPOSEVALUE (h_p, round_p, v)
69:     proposal ← v
70:     broadcast ⟨PROPOSAL, h_p, round_p, proposal, -1⟩
```

</td>
</tr>
</table>


### Notes

Most of the state transitions represented in the previous tables consider message and
events referring to the process' current round `r`.
In the pseudocode this current round of a process is referred as `round_p`.

There are however exceptions that have to be handled properly:
- the transition `L28` requires the process to have access to `PREVOTE` messages from a previous round `r' < r`.
- the transition `L49` requires the process to have access to `PRECOMMIT` messages from different round `r' != r`.
- the transition `L55` requires the process to have access to all messages from a future round `r' > r`.

[quint-spec]: ./quint/README.md
[quint-votekeeper]: ./quint/votekeeper.qnt
[quint-driver]: ./quint/driver.qnt
[quint-sm]: ./quint/consensus.qnt
