# Analysis of the "Starknet Forced Staking Updates" Specification

This document contains a report on analysis of the protocol specification of "Starknet Forced Staking Updates". See the [English specification](https://github.com/informalsystems/malachite/blob/main/specs/english/forced-updates/README.md) for an overview, and the Quint specifications in this directory for details. The specification follows [design document](https://docs.google.com/document/d/1OaYLh9o10DIsGpW0GTRhWl-IJiVyjRsy7UttHs9_1Fw/edit#heading=h.hvyiqqxvuqzo) prepared by Starkware.

When not indicated differently, we used `quint run resetTest.qnt` to conduct random simulation, and check properties for the default state machine (`init` and `step` actions). For some properties we used different transition relations (that is, different `step` actions.)

## Non-standard transition relations

In addition to the standard `step` action, we have added some more actions to model  environments that deviate from the expected behavior.

The following actions reduce the number of possible behaviors compared to  the standard `step` action:

- `stepNoRegs`: No registrations are ever added. While the standard step uses `addRegistration`, this is omitted here. This is done to highlight that liveness of the protocol depends to a large extent to the fact that registrations are continuously added ("infinitely often"). While we do not check liveness conditions yet, we observe the consequence by the fact that many witnesses that are reached under the standard step action cannot be reached here.
    - **TODO**: we might encode a transition systems where there are some registrations added, but from some time on, registrations stop being added.
- `stepProvableL2BlocksOnly`: all blocks added to L2 are provable. The standard step uses `addL2Block`, which non-deterministically sets a block provable or not.

The following actions add faulty behaviors to the standard `step` action:

- `stepWithPotentiallyOldProofs` and `stepWithPotentiallyFutureProofs`: these actions try to add proofs to L1 that do not match the height of the last proof submitted to L1. The standard step uses `addL1Block` that computes a proof always from the correct height (but potentially submits an invalid proof instead).
- `stepWithInvalidRegs`: This adds registrations to L2 that are not coming from L1.



## Invariants checked with quint run 



- Local L1 invariants
    - `noStaleWithProofInv`: If a valid proof was verified on L1, then there should be no stale updates
    - `provenHeightMonotonic`: latest L2 proven height in L1 blocks is non-decreasing
    - `L1ForkIDMonotonic`: L2 forkID in L1 blocks is non-decreasing

 - Proofs validation invariants
    - `InvalidRegistrationProofRejectedInv`: If the latest block in L1 does not include a (valid) proof or the proof contains an invalid registration, then the proof should be rejected. We check that by attesting that L1's provenHeight remains unchanged  (checked also for `--step "stepWithInvalidRegs"`)
    - `OldProofRejectedInv`: L1 blocks should not accept proofs with non monotonically increasing proven L2 heights. As a consequence, the latest L2 proven height in L1 should remain unchanged with such a proof is submitted (checked also with `--step stepWithPotentiallyOldProofs`)
    - `FutureProofRejectedInv`: If the proof starts from a block with height greater than provenHeight + 1 it is rejected. (checked also with `--step stepWithPotentiallyFutureProofs`)

- Local L2 invariants
    - `monotonicForkIDInv`: ForkID on L2 is non-decreasing
    - `monotonicStagedSeqInv`: the `highest_staged_seq_num` variable on L2 blocks is non-decreasing. This variable stores the sequenced number of the latest registration that is staged in L2.
    - `stagedInv`: we only have unstaged registrations which have seq_num greater than `highest_staged_seq_num`. This means, in particular, that we don't accept (unstage) duplicated registrations.
    - `strictlyMonotonicHeightInv`: L2 blocks' heights are strictly monotonic


- System-level invariants
    - `proofStateInv`: L1 stores hashes of L2 blocks that have been proven. This invariant checks if the latest proven L2 block stored in L1 matches the actual L2 block at `provenHeight`.
    - `forkIDNotSmaller`: L1 never expects a smaller forkID than there currently is on L2. (Note that it can be greater, if L1 expects a reset, and the reset did not happen yet at L2.)
          - **TODO**: create an action producing a spurious forkID (that differs from what L1 expects), and encode a witness that rejects a proof for such a block
    - `finalizationInv`: the latest L2 proven height stored in L1 cannot be bigger than L2's height. This is evident in the normal case (no forks), as proofs of L2 blocks are sent to L1 with some delay, and meanwhile more blocks can be added to L2. In case of resets, L2 height should match, but not be smaller than the latest L2 proven height stored in L1.
    - `oneForkIDperProofInv`: all L2 blocks that are proven within one proof submitted and accepted by L1 have the same forkID
    - `atMostOneResetPerForkIDInv`: L2 chain shouldn't roll back twice one same forkID. Upon each reset, L2 should use a different forkID. When there is a reset, L2 rolls back to a previous state (height). A hypothetical second reset, from the same forkID, would produce a second roll-back, which would be identified by this invariant. 
    - `noProvenRegistrationsUnfulfilledInv`: If a registration is included in a L2 block and the L2 block's proof is accepted by L1, the registration should not be in the `unfulfilled_updates` set maintained by L1.


## Interesting scenarios (witnesses)

We used `quint run resetTest.qnt` with the `--invariant`. In contrast to the invariants above, that we want to hold, the properties here we want to be violated. In case of a violation Quint provides us with a trace to leads to the violation, that is, the trace ends in an interesting state (that is defined by the negation of the property; in the text below we describe the reached state directly).

As example consider the witness `staleWitness` from the list below. The encoding in Quint is very simple and says that "the last block in L1 does not contain stale registrations". When we ask Quint whether this is an invariant, it tells us "violation" and provides a trace, where the last block in L1 contains a stale witness.

- `ProofAcceptedWitness`: generates a trace where the proof submitted to L1 was accepted
    - **TODO**: we might want to also encode different witnesses for different reasons why a proof was rejected
- `unstagedRegConfirmedWitness`: generates a trace where a registration is confirmed on L1 but still staged or unstaged on L2.
- `staleWitness`: generates a trace where the last block on L1 contains a stale registration
- `resetWitness`: generates a trace where there is a reset in L2, showing the first block after the reset, which has a forkID > 0
- `resetAfterProofWitness`: as above, while ensuring that before the reset a proof was accepted on L1 (i.e., provenHeight > 0)
- `forkProvedWitness`: generates a trace where a block produced by L2 after a fork is accepted on L1
- `ConfirmedWitness`: generates a trace where in the last L1 block a registration was confirmed
- `ProofNotAcceptedWitness`: generates a trace where the proof submitted to L1 was not accepted
- `unsuccessfulResetWitness`: generates a trace where there was a reset on L2, and before a second block
was added to L2 with the same fork ID, another reset happened

- `lastPossibleL1BlockWitnessCandidate`: this is a corner case. Experiments showed that it does not exist (see discussion in the Quint file). We leave it here, as we think it might be interesting to think through the scenario to better understand the protocol. The idea was to generate a
trace where in the previous L1 block there where no stale registrations (timed-out  unfulfilled registrations), but the unfulfilled registrations from the previous block
would become stale in the new block (as the time progressed). In this scenario, the proof
comes in just in time and so the registrations actually do not become stale. 

- `ProofAfterStaleWitness`: trace where there were stale registrations, then a proof came, end then there were no stale registrations


- `processedRegConfirmedWitness`: generates a trace where a registration is confirmed on L1 but not any more in staged or unstaged (or it never has been in these sets in case of the registration was added into L2 in a fork block). This means that the registration is actually applied to the current L2's validator set.
- `processedRegConfirmedNoForkWitness`: similar to previous, but last L2 block is no fork block
- `processedRegConfirmedNoForkAtAllWitness`: similar to previous, but no fork happened
- `OldProofRejectedWitness`: A proof that starts from a smaller L2 height than the proven height stored on L1 gets rejected; needs `--step "stepWithPotentiallyOldProofs"`. Compare to the invariant `OldProofRejectedInv`

- `FutureProofRejectedWitness`. Similar as above with larger height; needs `--step "stepWithPotentiallyFutureProofs"`. Compare to the invariant `FutureProofRejectedInv`.





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

Observe that under `--step "stepNoRegs"` there are less behaviors than in the standard `step` action. As a consequence all the invariants we reported above also hold under `stepNoRegs`.

### Injected invalid registrations

- `InvalidRegReachesL1Witness` generates a trace ( with `--step "stepWithInvalidRegs"`, while with the standard step, it is an invariant) where an invalid registration reaches L1. 
- `InvalidRegistrationProofRejectedWitness` as above, but also asserts that proof is rejected

## Temporal properties

We did not analyze them yet.

## Inductive invariants

TODO: We need to experiment here.

<!--
`quint compile --target tlaplus --invariant "oneForkIDperProofInv" resetTest.qnt > resetTest.tla`
-->


## TODOs

- Think about a good order / characterization of invariants/witnesses. Currently the order in the list seems a bit random.

Observations: 
- the number of registrations in L2 block a limiting factor for the reset
- not captured here: Time between block creation and proof on L1 must be big enough to also have proof on L2


