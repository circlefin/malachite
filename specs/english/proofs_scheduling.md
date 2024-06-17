# Proofs Scheduling

The Starket architecture includes the figure of a [prover][starkprover], a node
that produces **proofs** for blocks committed to the blockchain, in order to attest
the correct processing of the transactions included in that block.

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

[starkprover]: https://docs.starknet.io/architecture-and-concepts/network-architecture/starknet-architecture-overview/#provers
