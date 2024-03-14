# System

We consider a composition of three components
- L1. A smart contract on Ethereum
- L2. distributed system of full nodes and validators running a BFT consensus engine
- PR. nodes running prover software (potentially on the same machines as the full nodes/validators)

Some aspects of the composition
- The validity property of consensus (which determines whether a block should be decided on in L2), is defined by L1 and PR: A block _b_ is valid iff L1 can successfully verify _PR(b)_
- L1 accepts two kinds of proofs, namely proving
    1. normal block production (no error condition)
    2. production of an initial block of a fork after reset
- **normal block production:** _PR(b)_ is a proof that _b_ was produced properly, including
    - the state transition encoded in _b_ is consistent with the transactions in the block (TODO: not sure. can be polished) and the complete history of transaction in the prefix of the blockchain (iteratively, that is, one can apply a proof of a block to the proof of the prefix)
    - other meta data consistency is met (the pending validator set changes are consistent with the received registrations; same chain id as previous block; lastblockID is hash of last block, etc.)
    - if the block contains transactions, it must also contain a proof
    - enough of the required validators, have signed the block. "Enough" as defined by the history of the blockchain and the epoched validator set changes (we can write this more precisely), 
- **fork block production:** similar to above but
    - different meta data constraints as the new chain id comes from the epochs of L1 (TODO: does there need to be an acknowledgement to L1 about the reception of a new chainID?)
    - the required signatures are defined by data from L1 and L2 (TODO: confirm) 
        - the last block of L2 proved to L1
        - stale registrations from L1; TODO: confirm: I guess they must appear as transactions in the block (so that they can be acked to L1), but in contrast to the normal flow, they must be applied instantaneously
        - COMMENT: if height _f_ is a fork block, then checking the "validity" based on block _f-1_ requires a different function -> implies complexity for light clients that read L2
    - TODO: Confirm: I guess this block is allowed to contain transactions even if it doesn't have a block. Follow-up: If there is a new fork, some of the proofs that have been done for the old fork are still usable (the proofs always point to the past). Are we thinking about storing and re-proposing them?


- The "required validators" is information that originates from L1, via so called registrations, and is enforced by L1
    - L1 uses L1->L2 messaging (with acknowledgements) to make sure that L2 is aware of all registrations
    - if acknowledgements time out (in terms of EVE epochs), a reset happens (validator nodes observe that and take action)
        - a reset means, that L1 stops accepting "normal block production proofs" and requires specific "fork block production proofs"
        - as these specific proofs **enforce** the first block to contain timed-out registrations and a new validator set (and corresponding signatures), **validity enforces a reconfiguration**.
    - intuitively, L1 observes whether all its registrations are mirrored on L2 (TODO: confirm, by checking the proof, L1 can check that a specific registration appeared in L2). Then the existence of a proof of block production implies that the correct validator set as defined by the registration is used (and there are enough signatures)

QUESTION: As there is epoched staking, I wonder why registrations are sent one-by-one. In principle they could be sent as a batch at the end of an EVE epoch. 
    - This will lead to slightly different behavior on L2, as the Starknet epochs are not synchronized with EVE
    - this would potentially simplify ordering of messages in L1->L2?
    - not sure whether number of L1->L2 messages is a concern. I think in Interchain staking they are not happy with so many transfers (we need to confirm with the hub team)