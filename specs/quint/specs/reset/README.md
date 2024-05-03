# Analysis of the "Starknet Forced Staking Updates" Specification

## Invariants checked with quint run 

Here we do random simulation and checked that the invariant holds for the default state machine (init, step)

- Local L1 invariants
    - `noStaleWithProofInv`
    - `provenHeightMonotonic`
    - `L1ForkIDMonotonic`
    - `InvalidRegistrationProofRejectedInv` (checked also for `--step "stepWithInvalidRegs"`)
    
- Local L2 invariants
    - `monotonicForkIDInv`
    - `monotonicStagedSeqInv`
    - `strictlyMonotonicHeightInv`
    - `stagedInv`

- System-level invariants
    - `proofStateInv`
    - `forkIDNotSmaller`
    - `finalizationInv`
    - `oneForkIDperProofInv`
    - `atMostOneResetPerForkIDInv`



## Interesting properties

Here we do random simulation to reach a violation. The resulting trace ends in an interesting state

- `staleWitness`
- `noResetWitness`
- `noConfirmedWitness`
- `allProofsAcceptedWitness`
- `unsuccessfulResetWitness`

### Injected invalid registrations

- `InvalidRegReachesL1Witness` (`--step "stepWithInvalidRegs"` generates a witness, while with the standard step, it is an invariant).

## Temporal properties

We did not analyze them yet.

## Inductive invariants

TODO: Discuss with Gabriela!

`quint compile --target tlaplus --invariant "oneForkIDperProofInv" resetTest.qnt > resetTest.tla`



## TODOs

Observations: 
- the number of registrations in L2 block a limiting factor for the reset
- not captured here: Time between block creation and proof on L1 must be big enough to also have proof on L2


