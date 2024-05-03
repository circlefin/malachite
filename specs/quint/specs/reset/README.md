# Analysis of the "Starknet Forced Staking Updates" Specification

## Invariants checked with quint run 

Here we do random simulation and checked that the invariant holds for the default state machine (init, step)

- Local L1 invariants
    - `noStaleWithProofInv`: If a valid proof was verified on L1, then there should be no unfulfilled updates
    - `provenHeightMonotonic`: provenHeight non-decreasing
    - `L1ForkIDMonotonic`: ForkID on L1 is non-decreasing
    - `InvalidRegistrationProofRejectedInv`: (checked also for `--step "stepWithInvalidRegs"`) If there is no (valid) proof or the proof contains an invalid registration, then the proof should be rejected (provenHeight should remain unchanged)

- Local L2 invariants
    - `monotonicForkIDInv`: ForkID on L2 is non-decreasing
    - `monotonicStagedSeqInv`: "highest staged" variable on L2 is non-decreasing
    - `strictlyMonotonicHeightInv`: L2 height strictly monotonic
    - `stagedInv`: we only have unstaged registrations which have seq_num greater than highest_staged_seq_num

- System-level invariants
    - `proofStateInv`: hash stored on L1 is consistent with corresponding L2 Block
    - `forkIDNotSmaller`: L1 never expects a smaller forkID than there currently is on L2
        (TODO: think about a spurious reset)
    - `finalizationInv`: L2 is never rolled-back below provenHeight stored on L1
    - `oneForkIDperProofInv`: all L2 blocks that are proven with one proof on L1, have the same forkID
    - `atMostOneResetPerForkIDInv`: L2 chain shouldn't roll back twice one same forkID 



## Interesting properties

Here we do random simulation to reach a violation. The resulting trace ends in an interesting state

- `staleWitness`: generates a trace where the last block on L1 contains a stale registration
- `ResetWitness`: generates a trace where the last block on L2 comes after a reset (new forkID)
- `ConfirmedWitness`: generates a trace where in the last L1 block a registration was confirmed
- `ProofNotAcceptedWitness`: generates a trace where the proof submitted to L1 was not accepted
- `ProofAcceptedWitness`: generates a trace where the proof submitted to L1 was accepted
- `unsuccessfulResetWitness`: generates a trace where there was a reset on L2, and before a second block
was added to L2 with the same fork ID, another reset happened
- `InvalidRegReachesL1Witness`

- `lastPossibleL1BlockWitness`: trace where in the previous L1 block there where no stale registrations(timed-out  unfulfilled registrations), but the unfulfilled registrations from the previous block
would become stale in the new block (as the time progressed). In this scenario, the proof
comes in just in time. The registrations actually don't become stale. 
TODO: this is a corner case. Experiments showed that it doesn't exists. See discussion in qnt file.

- `ProofAfterStaleWitness`: trace where there were stale registrations, then a proof came, end then there were no stale registrations

- `unstagedRegConfirmedWitness`: generates a trace where a registration is confirmed on L1 but still staged or unstaged on L2.
- `processedRegConfirmedWitness`: generates a trace where a registration is confirmed on L1 but not any more in staged or unstaged (or it never has been in these sets in case of the registration was added into L2 in a fork block)
- `processedRegConfirmedNoForkWitness`: similar to previous, but last L2 block is no fork block




### Injected invalid registrations

- `InvalidRegReachesL1Witness` generates a trace ( with `--step "stepWithInvalidRegs"`, while with the standard step, it is an invariant) where a proof is rejected on L1 because there is an invalid registration proven. 

## Temporal properties

We did not analyze them yet.

## Inductive invariants

TODO: Discuss with Gabriela!

`quint compile --target tlaplus --invariant "oneForkIDperProofInv" resetTest.qnt > resetTest.tla`



## TODOs

Observations: 
- the number of registrations in L2 block a limiting factor for the reset
- not captured here: Time between block creation and proof on L1 must be big enough to also have proof on L2


