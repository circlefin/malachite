# Starknet Forced Staking Updates Specification

We consider a composition of three components
- L1. a smart contract on Ethereum
- L2. distributed system of full nodes and validators running a BFT consensus engine
- PR. nodes running prover software (potentially on the same machines as the full nodes/validators). That produce proofs to be sent to L1 (proofs that are stored on L2 are handled somewhere else, TODO: add pointer)

#### Outline of the protocol
The rough idea of the staking protocol is a follows:
- The stake is managed on the L1 staking registry. When a new staking event, called registration, happens, a message is sent from L1 to L2
- This message: 
    - results in a deferred update of the L2 validator set based on Starknet Validator Epochs SVE. If a registration is received by L2 in epoch _E_, it will affect the validator set of epoch _E+2_.
    - must be confirmed within a timeout. The timeout is defined with respect to Ethereum Validator Epochs EVE on L1. Think of them in the order of a day. If a registration times out, a reset must happen (details follow below).


## Overview

L2 uses L1 for security. This has two aspects:
1. proofs of L2 block production are submitted to and verified on L1. Once an L1 block with a valid proof becomes finalized, the L2 state becomes finalized. Thus, if clients wait for finalization of L1, they get the full security of L1
2. before L2 blocks are finalized, they are secured by a proof-of-stake mechanism for L2. By bonding stake on L1, validators have the incentive to follow the protocol.
    - **the goal of this protocol is to enforce the adoption of a new validator set, produced by L1**
    - for this to work, every change in the bonded stake on L1, so-called registrations, need to be reliably transmitted to L2
    - this is enforced by a timeout mechanism based on EVE epoch (say a day) and Point 1.: intuitively, L1 will only accept proofs for a L2 block B, if all registration from two days ago have been included in the L2 blocks up to B; if a timeout has expired, L1 enforces an L2 reset, by requiring a proof for a specific block that contains all registrations from two days ago, and a new forkID.

Notice, however, that there is no explicit signalization from L1 to start the reset protocol. Instead, validators that remain in the reset validator set and nodes that become validators in the reset validator set are expected to initiate the Fork Protocol, once they realize that it is needed. It is assumed that nodes joining the validator set of a fork have access to all state they need to produce and validate blocks (i.e., make progress) in that fork. 

For all effects, it can be useful to consider the first block of a new fork as it was a genesis state or block.

If L2 made progress until height 1000, but the last accepted proof on L1 was for height 900, on L2 this effectively means that correct validators need to roll-back to the state of 900 for the reset, and dismiss the previously produced blocks.

**Requirement.** In addition to ensure safety (every proof accepted on L1 contains all sufficiently old registrations), the protocol should ensure progress in favorable situations, that is: If at the end of an EVE epoch the validator set defined by L1 registrations contains a quorum of honest validators that are alive for sufficiently long, new blocks should be added to L2, and if there are alive provers, proofs should be added to L1.


## Central aspects of the composition
The validity property of consensus (which determines whether a specific block can be decided on in L2), is defined by L1 and PR: **A block _b_ produced by L2 is valid iff L1 can successfully verify _PR(b)_**
- _PR(b)_ actually stands for a combined proof of multiple L2 blocks. In practice, not every block is proven individually to L1
- validity is dependent on time; in particular the time on Ethereum. A block that is valid now, can become invalid if it takes too long to get a proof on L1. (This is due to stale registrations introduced below)

### Proofs
L1 accepts proofs for the block generation function. This function, roughly speaking, has two branches:
1. normal block production (no error condition)
2. production of an initial block of a fork after reset

#### Normal block production:
_PR(b)_ is a proof that _b_ was produced properly, including:
- the state transition encoded in _b_ is consistent with the transactions in the block, and the complete history of transaction in the prefix of the blockchain (iteratively, that is, one can apply a proof of a block to the proof of the prefix)
- other meta data consistency is met (the staged and unstaged validator set changes are consistent with the received registrations; same forkID as previous block; lastblockID is hash of last block, etc.)
- if the block contains transactions, it must also contain a proof (TODO: more details to come out of proof specification work that happens in parallel)
- a quorum of validators has signed the block. "Quorum" is defined by the history of the blockchain and the epoched validator set changes (we can write this more precisely)

**Observation** assumption/design decision: full nodes (validators) can check this kind of validity by observing only L2 (this doesn't mean that this is the validity that L1 is going to use in case there is a fork).

#### Fork block production:
Similar to above but:
- different meta data constraints, e.g. the new forkID comes from the stale registrations of L1 
- the new validator set is defined by data from L1 and L2 
    - the last block of L2 proved to L1 (validator set, staged and unstaged updates; TODO: clarify with Starkware)
    - stale registrations from L1; 
        - they must appear as transactions in the L2 block (so that L1 can verify they have been handled; TODO: verify with Starknet), 
        - in contrast to the normal flow, they must be applied instantaneously (to the metadata, that is, the validator set)

**Observation** assumption/design decision: full nodes (validators) need to observe L1 (stale registrations, last proven block) and L2 for this.


### Registrations
The "required validators" is information that originates from L1, via so called registrations, and is enforced by L1
- L1 uses L1->L2 messaging (with acknowledgements) to make sure that L2 is aware of all registrations
- if acknowledgements time out (in terms of EVE epochs), a reset happens (L2 validator nodes observe that and take action)
    - a reset means, that L1 stops accepting "normal block production proofs" and requires specific "fork block production proofs"
    - as these specific proofs **enforce** the first block to contain timed-out registrations and a new validator set (and corresponding signatures), and a new forkID, **validity enforces a reconfiguration**
- intuitively, L1 observes (via results that come with proofs) whether all its registrations are mirrored on L2. Then the existence of a proof of block production implies that the correct validator set as defined by the registration is used (and there are enough signatures)


### L1->L2 messaging
L1->L2 messaging is done by an oracle flow (not the IBC way of cryptographic proofs): the proposer sees a message to be sent on L1. When it can be sure that the other validators also have seen the message it puts it into the proposal, and the validators vote on it. This means, for validating a proposal, a validator needs to closely follow what happens on L1.

## Formalizing the protocol in Quint

We have formalized the reset protocol in Quint. To do so, we abstracted away many details not relevant to the understanding of the protocol. The specification includes:

- protocol functionality: how data inside blocks is computed and validated
- state machine consisting of L1, L2, and a set collecting registrations 
- invariants (that have been preliminarily tested) and temporal formulas (that are just written but have not been investigated further)

### Protocol functionality

This contains mainly the following functions (and their auxiliary functions):
- `pure def newL1Block (prev: L1Block, regs: Set[Registration], proof: L2Proof, delay: Time) : L1Block`
    - this returns a new L1 block, based on the previous block, newly added registrations, potentially a submitted proof for several L2 blocks, and a delay parameter the defines the time difference between the new block and the old one, to model progress in time
    - this function uses the crucial function `proofOK` to check whether the submitted proof can be verified. This captures central functionality for the rest protocol, namely whether the proof 
        - is for the right heights and forkID, and 
        - has all required unfulfilled updates.
- `pure def newL2Block (chain: List[L2Block], regs: Set[Registration]) : L2Block`
    - this returns a new L2 block during normal operation, based on the previous block and newly added registrations (that should be thought of having received via L1->L2 messaging)
    - it contains a branch with the following cases:
        - a new block within an SVE epoch or
        - a new block for a new SVE epoch
- `pure def forkBlock (prev: L2Block, regs: Set[Registration], h: Height, fID: ForkID) : L2Block`
    - this returns a new L2 block in the case of a reset. In addition to the "normal" parameters, it needs the last provenHeight and the new forkID which is information that the validators need to obtain from data on L1

- `pure def makeProof (l2: List[L2Block], to_height: Height, l1: List[L1Block]) : L2Proof`
    - This returns our abstraction of a proof of multiple L2 blocks. `L2Proof` is a sum-type to allow invalid and absent proofs
    - The function needs 
        - data from L2 to compute the result of confirmed registration
        - data from L1 namely the provenHeight

### State Machine

The state machine contains the following variables:
```
var L1: List[L1Block] 
var L2: List[L2Block]
var envRegs: Set[Registration]
```

In addition to several parameterized actions that we can use to control the creation of specific scenarios, we have the following actions to generate random traces:

- `addRegistration`
- `addL1Block`
- `addL2Block`
- `reset`

#### addRegistration
The addRegistration action creates a new registration with random content that is added to the state variable envRegs. This action represents the submission of registration from an external actor. The registration is not yet added to a L1 block.

#### addL1Block
The addL1Block action appends a new L1Block to the L1 blockchain (the state variable L1). The new L1 block includes all submitted registrations stored in the state variable envRegs, added to the  addL1Block fields newRegistrations and unfulfilled_updates. The unfulfilled_updates field contains the submitted but not yet confirmed (i.e., pending) registrations.

#### addL2Block
The addL2Block action appends a new L2Block to the L2 blockchain (the state variable L2). The new L2 block includes a random subset regs of the unfulfilled_updates field of the latest L1 block. Note that regs can be empty or its registrations may not follow the registration total order. The action uses the function `newL2Block` to compute the new block

#### reset
The reset action produces forks in the L2 blockchain when L2 fails to prove the inclusion in L2 blocks of the registrations produced by L1. There is a deadline, given in terms of L1 epochs, for each registration produced by L1 to be committed by L2. When the deadline for a registration is reached and the registration is still pending, i.e., it was not yet confirmed by L2, we say that the registration is stale. When there is a stale registration in L1, a fork should be produced in L2 as a way to enforce that all stale registrations are reflected in the validator set adopted by L2.

The reset action checks whether there are stale registrations in L1 by considering the last block appended to L1, and checking if there is any registration in pendingRegistrations whose submission epoch is older than two L1 epochs, considering as the reference, current epoch, the epoch of the last block appended to L1. If this is a case a fork is produced.

To produce a fork, the L2 blockchain is rolled-back to the latest (highest) L2's provenHeight. 
All L2 blocks with height higher than the latest L2's provenHeight are thus dropped. (In the Quint specification we currently store dropped blocks in the state variable `prevDeletedL2blocks` to inspect the reset scenarios completely)

Once L2 is rolled-back to latest L2's provenHeight, say h, a new block is appended to L2 with height h+1. This is a fork block produced by the forkBlock function. 

### Invariants and temporal formulas

For details we refer to the [state machine in Quint](./resetSystem.qnt), and the [analysis documentation](./quint.md).

## Issues

### Transfer registrations instead of valsets

QUESTION: As there is epoched staking, I wonder why registrations are sent one-by-one. In principle they could be sent as a batch at the end of an EVE epoch. 

- This will lead to slightly different behavior on L2, as the Starknet epochs are not synchronized with EVE
- this would potentially simplify ordering of messages in L1->L2?
- not sure whether number of L1->L2 messages is a concern. I think in Interchain staking they are not happy with so many transfers (we need to confirm with the hub team) -- but I think Starknet will do batches?
- as mentioned on Slack L1->L2 messaging from the past

### Lightclients

L2 Light clients are a concern. However, one needs to accept that they have reduced security compared to full nodes. In particular, we need to figure out whether and how a light client should figure out that there is a reset, and what to do in this case.

If height _f_ is a fork block, then checking the "validity" based on block _f-1_ requires a different function -> implies complexity for light clients that read L2; CONFIRM: are L2 light clients a concern? (i.e., validate state from L2)
 
### Re-using some proofs on L2

In general these proofs are handled somewhere else. But this point came up in discussions:

- Follow-up: If there is a new fork, some of the proofs that have been done for the old fork are still usable (the proofs always point to the past). Are we thinking about storing and re-proposing them?
