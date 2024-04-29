# Analysis of the "Starknet Forced Staking Updates" Specification


## Invariants checked with quint run 

### Default state machine (init, step)

Here we do random simulation and checked that the invariant holds

- `noStaleWithProofInv`
- `monotonicForkIDInv`
- `finalizationInv`
- `stagedInv`
- `oneForkIDperProofInv`
- `InvalidRegistrationProofRejectedInv` (checked also for `--step "stepWithInvalidRegs"`)
- `atMostOneResetPerForkIDInv`

## Interesting properties

Here we do random simulation to reach a violation. The resulting trace ends in an interesting state

- `staleWitness`
- `noResetWitness`
- `noConfirmedWitness`


### Injected invalid registrations


- `quint run --step "stepWithInvalidRegs" --invariant "InvalidRegReachesL1Witness" resetTest.qnt` generates a witness, while with the standard step, it is an invariant.




Observations: 
- the number of registrations in L2 block a limiting factor for the reset
- not captured here: Time between block creation and proof on L1 must be big enough to also have proof on L2
- TODO: L1->L2 messaging uses nonce for at-most-once delivery / but delivery might be out of order

