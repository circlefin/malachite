# Starknet Forced Staking Updates Specification

We consider a composition of three components
- L1. A smart contract on Ethereum
- L2. distributed system of full nodes and validators running a BFT consensus engine
- PR. nodes running prover software (potentially on the same machines as the full nodes/validators). That produce proofs to be sent to L1 (proofs that are stored on L2 are handled somewhere else, TODO: add pointer)

## Central aspects of the composition
The validity property of consensus (which determines whether a specific block can be decided on in L2), is defined by L1 and PR: **A block _b_ produced by L2 is valid iff L1 can successfully verify _PR(b)_**
    - _PR(b)_ actually stands for a combined proof of multiple L2 blocks. In practice not every block is proven individualy to L1
    - validity is dependent on time, in particular the time on Ethereum. A block that is valid now, can become invalid if it takes too long to get a proof on L1. (This is due to stale registrations introduced below)

### Proofs
 L1 accepts proofs for the block generation function. This function, roughly speaking, has two branches:
    1. normal block production (no error condition)
    2. production of an initial block of a fork after reset

#### Normal block production:
_PR(b)_ is a proof that _b_ was produced properly, including
    - the state transition encoded in _b_ is consistent with the transactions in the block (TODO: not sure. can be polished) and the complete history of transaction in the prefix of the blockchain (iteratively, that is, one can apply a proof of a block to the proof of the prefix)
    - other meta data consistency is met (the staged and unstaged validator set changes are consistent with the received registrations; same forkID as previous block; lastblockID is hash of last block, etc.)
    - if the block contains transactions, it must also contain a proof (TODO: more details to come out of proof specification work that happens in parallel)
    - a quorum of validators, have signed the block. "Quorum" is defined by the history of the blockchain and the epoched validator set changes (we can write this more precisely), 
    - **Observation** assumption/design decision: full nodes (validators) can check this kind of validity by observing only L2 (this doesn't mean that this is the validity that L1 is going to use in case there is a fork)

#### fork block production:
 similar to above but
    - different meta data constraints, e.g., as the new forkID comes from the stale registrations of L1 
    - the required signatures are defined by data from L1 and L2 
        - the last block of L2 proved to L1 (validator set, staged and unstaged updates)
        - stale registrations from L1; 
            - they must appear as transactions in the block (so that they can be acknowledged to L1), 
            - in contrast to the normal flow, they must be applied instantaneously (to the metadata, that is, the validator set)

**Observation** assumption/design decision: full nodes (validators) need to observe L1 (stale registrations, last proven block) and L2 for this.


### Registrations
The "required validators" is information that originates from L1, via so called registrations, and is enforced by L1
    - L1 uses L1->L2 messaging (with acknowledgements) to make sure that L2 is aware of all registrations
    - if acknowledgements time out (in terms of EVE epochs), a reset happens (L2 validator nodes observe that and take action)
        - a reset means, that L1 stops accepting "normal block production proofs" and requires specific "fork block production proofs"
        - as these specific proofs **enforce** the first block to contain timed-out registrations and a new validator set (and corresponding signatures), and a new forkID, **validity enforces a reconfiguration**.
    - intuitively, L1 observes (via results that come with proofs) whether all its registrations are mirrored on L2. Then the existence of a proof of block production implies that the correct validator set as defined by the registration is used (and there are enough signatures)


### L1->L2 messaging
L1->L2 messaging is done by an oracle flow (not the IBC way of cryptographic proofs): the proposer sees a message to be sent on L1. When it can be sure that the other validators also have seen the message it puts it into the proposal, and the validators vote on it. This means, for validating a proposal, a validator needs to closely follow what happens on L1.



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
