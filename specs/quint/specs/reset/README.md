# Analysis of the "Starknet Forced Staking Updates" Specification



## Invariants checked with quint run 

We used `quint run` to conduct random simulation, and checked that the invariant holds for the default state machine (`init`, `step`)

- Local L1 invariants
    - `noStaleWithProofInv`: If a valid proof was verified on L1, then there should be no unfulfilled updates
    - `provenHeightMonotonic`: latest L2 proven height in L1 blocks is non-decreasing
    - `L1ForkIDMonotonic`: L2 forkID in L1 blocks is non-decreasing
    - `InvalidRegistrationProofRejectedInv`: If the latest block in L1 does not include a (valid) proof or the proof contains an invalid registration, then the proof should be rejected. We check that by attesting that L1's provenHeight remains unchanged  (checked also for `--step "stepWithInvalidRegs"`)
    - `OldProofRejectedInv`: L1 blocks should not accept proofs with non monotonically increasing proven L2 heights. As a consequence, the latest L2 proven height in L1 should remain unchanged with such a proof is submitted (checked also with `--step stepWithPotentiallyOldProofs`)
    - `FutureProofRejectedInv`: If the proof starts from a block with height greater than provenHeight + 1 it is rejected. (checked also with step `stepWithPotentiallyFutureProofs`)

- Local L2 invariants
    - `monotonicForkIDInv`: ForkID on L2 is non-decreasing
    - `monotonicStagedSeqInv`: the `highest_staged_seq_num` variable on L2 blocks is non-decreasing. This variable stores the sequenced number of the latest registration that is staged in L2.
    - `strictlyMonotonicHeightInv`: L2 blocks' heights are strictly monotonic
    - `stagedInv`: we only have unstaged registrations which have seq_num greater than `highest_staged_seq_num`. This means, in particular, that we don't accept (unstage) duplicated registrations.

- System-level invariants
    - `proofStateInv`: hash stored on L1 is consistent with corresponding L2 Block
    - `forkIDNotSmaller`: L1 never expects a smaller forkID than there currently is on L2
        (TODO: think about a spurious reset)
    - `finalizationInv`: L2 is never rolled-back below provenHeight stored on L1
    - `oneForkIDperProofInv`: all L2 blocks that are proven within one proof on L1, have the same forkID
    - `atMostOneResetPerForkIDInv`: L2 chain shouldn't roll back twice one same forkID 
    - `noProvenRegistrationsUnfulfilledInv`: If a registration is in the proven prefix of L2, it must not be unfulfilled on L1

This also means that the invariants hold under `--step "stepNoRegs"` (as there are less behaviors).

## Interesting properties

We used `quint run` so that random simulation reaches a violation of the properties. The resulting trace ends in an interesting state (that is defined by the negation of the property; in the text below we describe the reached state directly)

- `staleWitness`: generates a trace where the last block on L1 contains a stale registration
- `resetWitness`: generates a trace where the last block on L2 comes after a reset (new forkID)
- `resetAfterProofWitness`: as above, but before the reset a proof was accepted on L1 (i.e., provenHeight > 0)
- `forkProvedWitness`: generates a trace where a block produced by L2 after a fork is accepted on L1
- `ConfirmedWitness`: generates a trace where in the last L1 block a registration was confirmed
- `ProofNotAcceptedWitness`: generates a trace where the proof submitted to L1 was not accepted
- `ProofAcceptedWitness`: generates a trace where the proof submitted to L1 was accepted
- `unsuccessfulResetWitness`: generates a trace where there was a reset on L2, and before a second block
was added to L2 with the same fork ID, another reset happened

- `lastPossibleL1BlockWitnessCanidate`: trace where in the previous L1 block there where no stale registrations(timed-out  unfulfilled registrations), but the unfulfilled registrations from the previous block
would become stale in the new block (as the time progressed). In this scenario, the proof
comes in just in time. The registrations actually don't become stale. 
TODO: this is a corner case. Experiments showed that it doesn't exists. See discussion in qnt file.

- `ProofAfterStaleWitness`: trace where there were stale registrations, then a proof came, end then there were no stale registrations

- `unstagedRegConfirmedWitness`: generates a trace where a registration is confirmed on L1 but still staged or unstaged on L2.
- `processedRegConfirmedWitness`: generates a trace where a registration is confirmed on L1 but not any more in staged or unstaged (or it never has been in these sets in case of the registration was added into L2 in a fork block). This means that the registration is actually applied to the current L2's validator set.
- `processedRegConfirmedNoForkWitness`: similar to previous, but last L2 block is no fork block

- `OldProofRejectedWitness`: A proof that starts from a smaller L2 height than the proven height stored on L1 gets rejected; needs `--step "stepWithPotentiallyOldProofs"`

- `FutureProofRejectedWitness`. Similar as above with larger height; needs `--step "stepWithPotentiallyFutureProofs"`

### No registrations

Registrations are crucial for progress. Using `--step "stepNoRegs"` we can generate traces without registrations. We see that the following witnesses from above actually don't appear:
- `staleWitness` 
- `ResetWitness`
- `forkProvedWitness`
- `ConfirmedWitness`
- `ProofNotAcceptedWitness` (No registrations can become stale, and the property doesn't capture non-accepted invalid proofs, or no proofs)
- `unsuccessfulResetWitness`

The main reason is that without registrations there are no resets, and all witness that are linked to resets cannot be reproduced.

The witness `ProofAcceptedWitness` still works without registrations.

### Injected invalid registrations

- `InvalidRegReachesL1Witness` generates a trace ( with `--step "stepWithInvalidRegs"`, while with the standard step, it is an invariant) where an invalid registration reaches L1. 
- `InvalidRegistrationProofRejectedWitness` as above, but also asserts that proof is rejected

## Temporal properties

We did not analyze them yet.

## Inductive invariants

TODO: Discuss with Gabriela!

`quint compile --target tlaplus --invariant "oneForkIDperProofInv" resetTest.qnt > resetTest.tla`



## TODOs

Observations: 
- the number of registrations in L2 block a limiting factor for the reset
- not captured here: Time between block creation and proof on L1 must be big enough to also have proof on L2


