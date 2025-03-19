# Tendermint Consensus Algorithm: Pseudo-code

Algorithm 1 from page 6 of the paper
["The latest gossip on BFT consensus"][tendermint-arxiv]
([PDF][tendermint-pdf]), by Ethan Buchman, Jae Kwon,
and Zarko Milosevic, last revised in November 2019:
Tendermint consensus algorithm.

```go
 1: Initialization:
 2:    h_p := 0     /* current height, or consensus instance we are currently executing */
 3:    round_p := 0 /* current round number */
 4:    step_p ∈ {propose, prevote, precommit}
 5:    decision_p[] := nil
 6:    lockedValue_p := nil
 7:    lockedRound_p := −1
 8:    validValue_p := nil
 9:    validRound_p := −1

10: upon start do StartRound(0)

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
21:       schedule OnTimeoutPropose(h_p, round_p) to be executed after timeoutPropose(round_p)

22: upon ⟨PROPOSAL, h_p, round_p, v, −1⟩ from proposer(h_p, round_p)
    while step_p = propose do
23:    if valid(v) ∧ (lockedRound_p = −1 ∨ lockedValue_p = v) then
24:       broadcast ⟨PREVOTE, h_p, round_p, id(v)⟩
25:    else
26:       broadcast ⟨PREVOTE, h_p, round_p, nil⟩
27:    step_p ← prevote

28: upon ⟨PROPOSAL, h_p, round_p, v, vr⟩ from proposer(h_p, round_p) AND 2f + 1 ⟨PREVOTE, h_p, vr, id(v)⟩
    while step_p = propose ∧ (vr ≥ 0 ∧ vr < round_p) do
29:    if valid(v) ∧ (lockedRound_p ≤ vr ∨ lockedValue_p = v) then
30:       broadcast ⟨PREVOTE, h_p, round_p, id(v)⟩
31:    else
32:       broadcast ⟨PREVOTE, h_p, round_p, nil⟩
33:    step_p ← prevote

34: upon 2f + 1 ⟨PREVOTE, h_p, round_p, ∗⟩ while step_p = prevote for the first time do
35:    schedule OnTimeoutPrevote(h_p, round_p) to be executed after timeoutPrevote(round_p)

36: upon ⟨PROPOSAL, h_p, round_p, v, ∗⟩ from proposer(h_p, round_p) AND 2f + 1 ⟨PREVOTE, h_p, round_p, id(v)⟩
    while valid(v) ∧ step_p ≥ prevote for the first time do
37:    if step_p = prevote then
38:       lockedValue_p ← v
39:       lockedRound_p ← round_p
40:       broadcast ⟨PRECOMMIT, h_p, round_p, id(v))⟩
41:       step_p ← precommit
42:    validValue_p ← v
43:    validRound_p ← round_p

44: upon 2f + 1 ⟨PREVOTE, h_p, round_p, nil⟩ while step_p = prevote do
45:    broadcast ⟨PRECOMMIT, h_p, round_p, nil⟩
46:    step_p ← precommit

47: upon 2f + 1 ⟨PRECOMMIT, h_p, round_p, ∗⟩ for the first time do
48:    schedule OnTimeoutPrecommit(h_p, round_p) to be executed after timeoutPrecommit(round_p)

49: upon ⟨PROPOSAL, h_p, r, v, ∗⟩ from proposer(h_p, r) AND 2f + 1 ⟨PRECOMMIT, h_p, r, id(v)⟩
    while decision_p[h_p] = nil do
50:    if valid(v) then
51:       decision_p[h_p] = v
52:       h_p ← h_p + 1
53:       reset lockedRound_p, lockedValue_p, validRound_p and validValue_p to initial values
54:       StartRound(0)

55: upon f + 1 ⟨∗, h_p, round, ∗⟩ with round > round_p do
56:    StartRound(round)

57: Function OnTimeoutPropose(height, round):
58:    if height = h_p ∧ round = round_p ∧ step_p = propose then
59:       broadcast ⟨PREVOTE, h_p, round_p, nil⟩
60:       step_p ← prevote

61: Function OnTimeoutPrevote(height, round):
62:    if height = h_p ∧ round = round_p ∧ step_p = prevote then
63:       broadcast ⟨PRECOMMIT, h_p, round_p, nil⟩
64:       step_p ← precommit

65: Function OnTimeoutPrecommit(height, round):
66:    if height = h_p ∧ round = round_p then
67:       StartRound(round_p + 1)
```

The [overview.md](./overview.md) document details the operation of Tendermint
consensus algorithm, from its pseudo-code.

[tendermint-arxiv]: https://arxiv.org/abs/1807.04938
[tendermint-pdf]: https://arxiv.org/pdf/1807.04938
