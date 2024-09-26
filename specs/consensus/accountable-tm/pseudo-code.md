# Pseudo Code Changes in Accountable Tendermint

Changes from the [original pseudo-code](../pseudo-code.md) of Tendermint consensus algorithm.

#### Line 22

``` go
22: upon ⟨PROPOSAL, h_p, round_p, v, −1⟩ from proposer(h_p, round_p) while step_p = propose do
23:    if valid(v) ∧ (lockedRound_p = −1 ∨ lockedValue_p = v) then
24:       broadcast ⟨PREVOTE, h_p, round_p, id(v), -1⟩
25:    else
26:       broadcast ⟨PREVOTE, h_p, round_p, nil, -1⟩
27:    step_p ← prevote
```


#### Line 28

``` go
28: upon ⟨PROPOSAL, h_p, round_p, v, vr⟩ from proposer(h_p, round_p) AND 2f + 1 ⟨PREVOTE, h_p, vr, id(v), vr'⟩
    while step_p = propose ∧ (vr ≥ 0 ∧ vr < round_p) do
29:    if valid(v) ∧ (lockedRound_p ≤ vr ∨ lockedValue_p = v) then
30:       broadcast ⟨PREVOTE, h_p, round_p, id(v), vr⟩
31:    else
32:       broadcast ⟨PREVOTE, h_p, round_p, nil, vr⟩
33:    step_p ← prevote
```

#### Line 34

```go
34: upon 2f + 1 ⟨PREVOTE, h_p, round_p, ∗, ∗⟩ while step_p = prevote for the first time do
35:    schedule OnTimeoutPrevote(h_p, round_p) to be executed after timeoutPrevote(round_p)
```

#### Line 36

``` go
36: upon ⟨PROPOSAL, h_p, round_p, v, vr⟩ from proposer(h_p, round_p) AND 2f + 1 ⟨PREVOTE, h_p, round_p, id(v), vr⟩ while
    valid(v) ∧ step_p ≥ prevote for the first time do
37:    if step_p = prevote then
38:       lockedValue_p ← v
39:       lockedRound_p ← round_p
40:       broadcast ⟨PRECOMMIT, h_p, round_p, id(v))⟩
41:       step_p ← precommit
42:    validValue_p ← v
43:    validRound_p ← round_p

```

#### Line 44

```go
44: upon 2f + 1 ⟨PREVOTE, h_p, round_p, nil⟩ while step_p = prevote do
45:    broadcast ⟨PRECOMMIT, h_p, round_p, nil, -1⟩
46:    step_p ← precommit
```

#### Line 57

```go
57: Function OnTimeoutPropose(height, round):
58:    if height = h_p ∧ round = round_p ∧ step_p = prevote then
59:       broadcast ⟨PREVOTE, h_p, round_p, nil, -1⟩
60:       step_p ← prevote
```
