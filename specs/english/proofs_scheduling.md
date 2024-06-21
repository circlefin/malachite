# Proofs Scheduling

The Starket architecture includes the figure of a [prover][starkprover], a node
that produces **proofs** for blocks committed to the blockchain, in order to attest
the correct processing of the transactions included in that block.


## Overview

Since **producing proofs is slow**, we should expect the proof for a block to
take several blocks to be produced.
So once a block is committed at height `H` of the blockchain, a prover is
expected to take `L` time to produce a proof of block `H`.
Meanwhile, several blocks may have been committed to the blockchain, to that
the proof of block `H` will only be available at the time when a block is being
produced and proposed at a height `H' > H`.

Since **production proofs is expensive**, we should avoid having multiple
provers spending resources to proof the same block.
The need for a **scheduling protocol** derives from this requirement.
Of course, in bad scenarios, we would need multiple provers for a single block,
but this situation should be avoided whenever possible.

**Proofs are included in blocks** and are committed to the blockchain.
If fact, the content of block is only _final_ when:
(i) it is committed to the blockchain,
(ii) another block including its proof is also committed to the blockchain,
and (iii) the proof of the original block is sent to and validate by L1, the
Ethereum blockchain.

Ideally, each proposed block should include a proof of a single, previously
committed block.
However, we cannot guarantee that a proof of a previously committed block is
available whenever a new block is proposed. As a result, some blocks may not
include any proof, and some other blocks will need to include proofs of
multiple, previously committed blocks.
Or, more precisely, a proof attesting the proper execution of multiple blocks.

Since proofs are part of proposed blocks, the **prover** role in this
specification is associated to the **proposer** role in the consensus protocol.
The proposer a block at height `H` is expected to include in the proposed block
proofs of previous blocks.
How the right provers ship proofs to the right proposers is not considered in
this document.

> TODO: review relation between proposer and prover

## Strands

The proposed solution is to adopt a **static scheduling** protocol.
The blockchain is virtually split into a number of **strands**,
so that proofs of blocks belonging to a strand are included in blocks belonging
to the same strand.

Using `K` to denote the number of strands in the blockchain,
the mapping of blocks to strands is as follows:

- A block at height `H` of the blockchain belongs to the strand: `strand(H) = H mod K`.

The constant `K` should be defined considering a conservative upper bound for
the latency `L` to produce a proof for a block and expected block latency
(i.e., the expected interval between committing successive blocks).
The goal is to ensure, with high probability, that no more than `K` blocks are
produced and committed in `L` time units,
so that the proof of a block committed at height `H` is available when height
`H' = H + K` is started.

### Scheduling

The static strand-based scheduling is represented as follows.

Lets `proof(H)` be the proof of the block committed at height `H`, then:

- `proof(H)` is included in a block committed at height `H' = H + i * K`, with `i > 0`.

This is a **safety** property, stating that proofs of blocks and the proven
blocks are committed in the same strand.
In fact, since `strand(H) == strand(H + i * K)`, for any integer `i`, proofs
and blocks are in the same strand.

In the ideal, best-case scenario we have `i == 1`, meaning that the proof of the
block committed at height `H` is included in block `H' = H + K`.
If, for any reason, `proof(H)` is not available to the proposer of block `H'`
when it produces the block to propose in height `H'`, then the
inclusion of `proof(H)` is shifted to the next block in the same strand
`strand(H)`, which would be `H" = H' + K = H + 2 * K`.
This undesired scenario can be observed multiple times, resulting in another
shift by `K` on the block height where `proof(H)` is included.

We want to limit the number of blocks in a strand that do not include proofs.
So we define a constant `P` and state the following **liveness** property:

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
Nodes must know `valset(H)` before starting their participation in the
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

For the sake of the scheduling protocol, we distinguish between two kind of blocks:

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
A block is unproven when its proof was not yet committed to the blockchain.

If a proposer of height `H` is **able** produce or retrieve the expected set
of proofs, for all unproven blocks belonging to `strand(H)`, then it is allowed
to produce and propose a **full block**, i.e., a block containing transactions.

But if the proposer of height `H` is **not able** to produce or retrieve the
full expected set of proofs, then it is forced to produce an **empty block**,
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

The first property shows that, except for a corner scenario, there are always
proofs to be included in a new block:

- For all heights `H >= K`, there are always blocks to proof, i.e., `expected_proofs(H) != Ø`.

This happens because the previous height in the same strand `strand(H)`, height
`H - K >= 0`, has not yet been proven, as there is not height between `H - K`
and `H` belonging to the same strand as height `H`. 
As a corollary:

- For every strand `s`, either it has no blocks (i.e., blockchain height `< K`)
  or `unproven(s) != Ø`.

Considering now strands instead of heights, for every strand `s` we have:

1. The first (lowest) height `Hmin` in `unproven(s)` is of a block that
   contains **proofs**.
2. Every other height `H' > Hmin` in `unproven(s)` is of an **empty block**
   that does not contain proofs.
3. There are no gaps in `unproven(s)`, namely for every integer `i` with
   `0 <= i < |unproven(s)|`, the height `H(i) = Hmin + i * K` is
   present in `unproven(s)` and, of course, `strand(H(i)) == s`.
4. There is at most `P` heights of **empty blocks** in `unproven(s)`,
   by the [strand scheduling](#scheduling) definition.

These properties can be proved by induction on `unproven(s)` and the
strand-based static scheduling protocol.

The intuition is that when producing a new block on a strand `s`, say block
`H`, we have two possibilities:
(i) the proposer of block `H` includes in the block all unproven blocks on
strand `s`, therefore resetting `unproven(s)` to empty,
or (ii) produces an empty block with no proofs, thus leaving `unproven(s)`
unchanged.
Since new block `H` is not yet proven, as just committed, it is appended to
`unproven(s)`.

## Implementation

The proposed implementation for the previously described protocol works as follows.

When block `H` is committed to the blockchain, the prover of the next height in
strand `strand(H)` is expected to start generating a proof of block `H`.
The prover is either `proposer(H + K, 0)` or some node associated to it.

To generate the proof of block `H`, the prover needs the proof of the previous
block in strand `strand(H)`, whose height is `H - K`.
In the favorable scenario, `proof(H - K)` is included in block `H`, so the
production of `proof(H)` can start immediately.
Otherwise, the prover needs to compute `unproven(strand(H))` and follow the steps:

1. Go back to the block with the first (lowest) height `Hmin` in
   `unproven(strand(H))`, which must include proofs (by property 1.),
   and use `block(Hmin).proofs` and `block(Hmin)` to produce `proof(Hmin)`;
   - Notice that in the favorable scenario `H == Hmin`, and the process is done here.
2. Go to the block `Hmin + K` and use `proof(Hmin)` and  `block(Hmin + K)` to
   produce `proof(Hmin + K)`. This operation should be faster because
   `block(Hmin + K)` must be empty (by property 2.).
3. If `Hmin + K == H`, the process is done. Otherwise, set `Hmin = Hmin + K`
   and repeat step 2.

At the end of the process, the prover has produced **one** proof for a full
block, at height `Hmin`, and possibly **some** proofs for empty blocks.
All the produced proofs should be included in the block proposed at height `H + K`.

### Additional rounds

The implementation up to now considers that the primary proposer of a height
should schedule the production of proofs.
In other words, once height `H` starts, `proposer(H, 0)` is expected to have
`expected_proofs(H)`.

The proposers of other rounds, i.e., `proposer(H, R)` for round `R > 0`,
do not have the same requirement.
If they _happen_ to have `expected_proofs(H)`, they can produce and propose a
full block, including transactions and the required proofs.
Otherwise, which is consider the normal case, they will propose an **empty block**.

There is an exception for this mechanism intended to limit the number of
**empty blocks** in a strand.
So, if there are `P` empty blocks in the current strand `s`, namely  if
`|unproven(s)| > P`, the proposer of **any round** of a height `H` with
`strand(H) == s` can only propose a block if it includes `expected_proofs(H)`.

[starkprover]: https://docs.starknet.io/architecture-and-concepts/network-architecture/starknet-architecture-overview/#provers
