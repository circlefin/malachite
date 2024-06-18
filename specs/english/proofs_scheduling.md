# Proofs Scheduling

The Starket architecture includes the figure of a [prover][starkprover], a node
that produces **proofs** for blocks committed to the blockchain, in order to attest
the correct processing of the transactions included in that block.


## Overview

Since **producing proofs is slow**, we should expect the proof for a block to
take several blocks to be produced.
So once a block is committed at height `H` of the blockchain, we should not
expect a prover to produce the proof of block `H` before the time at which
the block `H' = H + L` is proposed.
The constant `L` should be computed by considering the expected (good case)
latency to produce a proof and the typical Starknet's block latency.

Since **production proofs is expensive**, we should avoid having multiple
provers spending resources to proof the same block.
The need for a **scheduling protocol** derives from this requirement.
Of course, in bad scenarios, we would need multiple provers for a single block,
but this scenario should be avoided whenever possible.

**Proofs are included in blocks** and are committed to the blockchain.
If fact, a block is only "finalized" when it is committed to the blockchain **and**
another, future block including its proofs is also committed to the blockchain.
Ideally, each proposed block should contain a proof of a previously committed block.
But it is possible to have either blocks with no proof included, as none was
available, and blocks with multiple proofs included.

Since proofs are part of proposed blocks, the **prover** role in this
specification is associated to the **proposer** role in the consensus protocol.
The proposer a block at height `H` is expected to include in the proposed block
proofs of previous blocks.
How the right provers ship proofs to the right proposers is not considered in
this document.


## Strands

The proposed solution is to adopt a **static scheduling** protocol.
The blockchain is virtually split into a number of strands,
so that proofs of blocks belonging to a strand are included in blocks belonging
to the same strand.

We define a constant `K`, which is the number of strands, and map blocks to
strands as follows:

- A block at height `H` of the blockchain belongs to the strand: `strand(H) = H mod K`.

The number `K` of strands should be chosen as a safe upper bound the latency `L`
for producing the proof of a block, given in terms of the number of blocks that
Starknet is expected to commit while the proof is produced.

### Scheduling

The static strand-based scheduling is represented as follows.

Lets `proof(H)` be the proof of the block committed at height `H`, then:

- `proof(H)` is included in a block committed at height `H' = H + i * K`, with `i > 0`.

Notice that `strand(H) == strand(H + i * K)`, where `i` is any integer.
The ideal, best-case scenario we have `i == 1`, meaning that the proof of the
block committed at height `H` is included in block `H' = H + K`.

If, for any reason, `proof(H)` is not ready when block `H'` is proposed, the
inclusion of `proof(H)` is shifted to the next block in the same strand
`strand(H)`, which would be `H" = H' + K = H + 2 * K`.
This bad scenario can be observed multiple times, resulting in another shift by
`K` on the block height where `proof(H)` is included.

However, we want to limit the number skipped blocks in a strand, so we define
a constant `P` and require:

- `proof(H)` must be included in a block committed up to height `H* = H + (P + 1) * K`.

So, if `proof(H)` is not included in blocks committed at heights `H' = H + i * K`,
with `1 <= i <= P`, then height `H*` cannot be concluded until the proposed
block that ends up being committed includes `proof(H)`.

## Context

Before detailing the proofs scheduling protocol implementation, we introduce
some minimal context.

### Consensus

The block committed to the height `H` of the blockchain is the value decided in
the instance `H` of the consensus protocol.
An instance of consensus consists of one or multiple rounds `R`, always
starting from round `R = 0`.
We expect most heights to be decided in the first round, so the scheduling
protocol focuses on this scenario.

The instance `H` of the consensus protocol is run by a set of validators
`valset(H)`, which is known by all nodes.
The same validator set is adopted in all rounds of a height, but the validator
set may change over heights.
Nodes must known `valset(H)` before starting their participation in the
instance `H` of the consensus protocol.

There is a deterministic function `proposer(H,R)` that defines from `valset(H)`
the validator that should propose a block in the round `R` of the instance `H`
of the consensus protocol.
We define, for the sake of the scheduling protocol, the **primary proposer** of
height `H` as the proposer of its first round, i.e., `proposer(H,0)`.

### Blocks

Blocks proposed in a round of the consensus protocol and eventually committed
to the blockchain are formed by:

- A **header** field, containing consensus and blockchain related information
- A **proofs** field, containing of a, possibly empty, set of proofs of previous blocks
- A **payload** field, consisting of a, possibly empty, set transactions submitted by users

For the sake of the scheduling protocol, we distinghish between two kind of blocks:

- **Full blocks** carry transactions, i.e., have a non-empty payload.
  The protocol requires full blocks to include proofs of previously committed blocks.
  Full blocks are the typical and relevant blocks in the blockchain.
- **Empty blocks** do not carry transactions, i.e., have an empty payload.
  The protocol may force the production of empty blocks, which are undesired,
  when their proposers do not have proofs to include in the block.


## Protocol

The proofs scheduling protocol specifies the behaviour of the **proposers** of
rounds of the consensus protocol.

### Overview

A proposer is expected to include in its proposed block for height `H`
**proofs for all unproven blocks** committed to the same strand as height `H`.
A block is unproven when its proof was not yet commmitted to the blockchain.

If a proposer of height `H` is **able** produce or retrieve the expected set
of proofs, for all unproven blocks belonging to `strand(H)`, then it is allowed
to produce and propose a **full block**, i.e., a block containing transactions.

But if the proposer of height `H` is **not able** to produce or retrieve the
full expected set of proofs, then it is forced to produce a **empty block**,
i.e., a block without transactions, with an empty **proofs** field.
Notice that the proposer does not include any proof on the block if it has only
_part_ of the proofs expected to be included in that block.

The reason for the last behaviour, forcing the production of empty blocks when
the expected set of proofs is not available, is to discourage the production of
blocks without proofs.
There are rewards for proposers that produce blocks that end-up committed,
associated to the transactions included in the block.
Producing an empty block is therefore not interesting for a proposer, that
should do its best to include all required proofs in produced blocks.

### Formalization

First, lets define what it is meant by unproven blocks in a strand `s` at a
given state of the blockchain:

- `unproven(s)` is a set of heights `H` with `strand(H) == s` and whose
  `proof(H)` was not yet committed.

Then, lets extend the definition of `proof(H)` to consider multiple proofs,
or proofs from a set `S` of heights:

- `proofs(S)` is a set containing a `proof(H)` for every height `H` in the set
  `S` of heights.

Finally, lets define the expected set of proofs to be included in the block at
height `H`:

    expected_proofs(H) = proofs(unproven(strand(H)))

So, lets `s = strand(H)`, the set of proofs expected to be included in block `H` 
is `proofs(unproven(s))`.

From the roles presented to the operation of a proposer of height `H`, we can
define the following **invariant**:

    block(H).payload != Ø => block(H).proofs == expected_proofs(H)

Namely, if the block carries a payload (transactions), then it must include all
the expected proofs for its height.

### Properties

**TODO**: define properties of strands, mostly already drafted below:

A

- For all heights `H < K`, `expected_proofs(H) == Ø`
- For all heights `H >= K`, `expected_proofs(H) != Ø`

B

If `expected_proofs(H) != Ø` then obviously `unproven(strand(H)) != Ø`.
Lets `s == strand(H)`, we have:

- Lets `Hmin` to be the minimum height present in `unproven(s)`, we have:
  - `block(Hmin).proofs != Ø`
  - `block(Hmin)` can be a **full block**, i.e., it can contain transactions
- For every `H' > Hmin` in `unproven(s)`, we have:
  - `block(H').proofs == Ø`
  - `block(H').payload == Ø`, i.e., block `H'` is an **empty block**.
- Every block `H'` with `strand(H') == s`, `H' >= Hmin` and `H' < H` is present in `unproven(s)`.
- Finally, `|unproven(s) / {Hmin}| <= P`

### Implementation

Primary proposer of height `H`, i.e., `proposer(H,0)` is expected to produce `expected_proofs(H)`.
Therefore it is expected to produce a full block.

Other proposers of height `H`, i.e., `proposer(H,R)` with `R > 0`, are not expected to produce `expected_proofs(H)`.
Therefore, they are expected to produce empty blocks.

The exception is when `|expected_proofs(H)| == P + 1`.
In this case, height `H` must commit a block including `expected_proofs(H)`, no matter how long it takes.

[starkprover]: https://docs.starknet.io/architecture-and-concepts/network-architecture/starknet-architecture-overview/#provers
