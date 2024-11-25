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

We want to limit the number of blocks in the strand `strand(H)` that do not include the proof of block `H`.
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

### Epochs

Starknet restricts how often the validator set can be updated by adopting the
concept _Starknet Validator Epochs_ (SVE).
An epoch `e` is a sequence of heights, with predefined length `E`, during which
the validator set adopted in consensus remains unchanged.
Moreover, the validator set to be adopted in epoch `e + 2` is defined when the
last block of epoch `e` is committed, i.e., when epoch `e + 1` is about to start.
More details it this [document](../validator-updates/README.md).

For the sake of proof scheduling, when a block at height `H` of epoch `e` is
committed, the validator set adopted in all heights of epoch `e` is known, and
so is the validator set to be adopted in all heights of epoch `e + 1`.
In the best case, `H` is the last height of epoch `e`, so that the validator
sets of the next two fulls epochs are known, i.e., the validator set is known
up to height `H + 2E`.
In the worst case, `H` is the penultimate height of epoch `e`, i.e., there is
still one height on the current epoch, and the validator set is known up to
height `H + 1 + E`.

As a result, if the system is formed by `K` strands then the schedule
algorithm implicitly requires `K <= E + 1`.
In this way, when the block at height `H` is committed, the validator set
`valset(H')` of the block `H' = H + K` where the proof of block `H` is expected
to be included is known by all system participants.

### Blocks

Blocks proposed in a round of the consensus protocol and eventually committed
to the blockchain are formed by:

- A `header` field, containing consensus and blockchain related information
- A `proof` field, possibly empty, containing a proof  of a set of previous blocks
- A `payload` field, possibly empty, consisting of a set transactions submitted by users

For the sake of the scheduling protocol, we distinguish between two kind of blocks:

- **Full blocks** carry transactions, i.e., have a non-empty `payload`.
  The protocol requires full blocks to include a non-empty `proof` field.
  Full blocks are the typical and relevant blocks in the blockchain.
- **Empty blocks** do not carry transactions, i.e., have an empty `payload`.
  The protocol may force the production of empty blocks, which are undesired,
  when their proposers do not have a proof to include in the block.


## Protocol

The proofs scheduling protocol specifies the behaviour of the **proposers** of
rounds of the consensus protocol.

### Overview

A proposer is expected to include in its proposed block at height `H` a
`proof` for **all unproven blocks** committed to the same strand as height `H`.
A block is unproven when its proof was not yet committed to the blockchain.

If a proposer of height `H` **has received**, from the designated provers, a
`proof` for all unproven blocks belonging to `strand(H)`, then it is allowed to
produce and propose a **full block**, i.e., a block containing transactions.

But if the proposer of height `H` **has not received**, from the designated provers, 
a `proof` for all unproven blocks belonging to `strand(H)`, then it is forced
to propose an **empty block**, i.e., a block without transactions, and with an
empty `proof` field.
Notice that the proposer may have received an _incomplete_ proof, proving only
part of the unproven blocks in the current strand, but only _full_ proofs can
be included in proposed blocks.

The reason for forcing the production of **empty blocks** when a proof for
**all unproven blocks** is **not available** is to discourage the production of
blocks with an empty `proof` field.
There are rewards for proposers that produce blocks that end-up committed,
associated to the transactions included in the block.
Producing an empty block is therefore not interesting for a proposer, that
should do its best to include _full_ proofs in the proposed blocks.

There is a second reason for enforcing this behavior, which is the fact that
producing a **proof for an empty block** should be **faster** and less
expensive than producing a proof for a full block.
Thus, if a block has an empty `proof` field, therefore does not contributes to
the proving mechanism, it should at least be easier to prove.

### Formalization

First, lets define what it is meant by unproven blocks in a strand `s` at a
given state of the blockchain:

- `unproven(s)` is a set of heights `H` with `strand(H) == s` and whose
  `proof(H)` was not yet committed.

Then, lets extend the definition of `proof(H)` to consider proofs for multiple
blocks, from a set `S` of heights:

- `proofs(S)` is a proof that includes a `proof(H)` for every height `H` in the set
  `S` of heights.

Finally, lets define the expected proof to be included in the block at
height `H`:

    expected_proof(H) = proofs(unproven(strand(H)))

So, lets `s = strand(H)`, the proof included in block `H` should prove all blocks
in `proofs(unproven(s))`.

From the roles presented to the operation of a proposer of height `H`, we can
define the following **invariant**:

    block(H).payload != Ø => block(H).proof == expected_proof(H)

Namely, if the block carries a payload (transactions), then it must include the
full expected proof for its height.

### Properties

The first property shows that, except for a corner scenario, there are always
proofs to be included in a new block:

- For all heights `H >= K`, there are always blocks to proof, i.e., `expected_proof(H) != Ø`.

This happens because the previous height in the same strand `strand(H)`, height
`H - K >= 0`, has not yet been proven, as there is not height between `H - K`
and `H` belonging to the same strand as height `H`. 
As a corollary:

- For every strand `s`, either it has no blocks (i.e., blockchain height `< K`)
  or `unproven(s) != Ø`.

Considering now strands instead of heights, for every strand `s` we have:

1. The first (lowest) height `Hmin` in `unproven(s)` is of a block that
   contains an non-empty `proof` field.
2. Every other height `H' > Hmin` in `unproven(s)` is of an **empty block**
   with an empty `proof` field.
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

This section presents an implementation for the previously described protocol.
More specifically, it defines the behaviour of **provers** and how they are
supposed to produce proofs for committed blocks.

When block `H` is committed to the blockchain, the **prover** of the next height in
strand `strand(H)` is expected to start generating a proof of block `H`.
Notice that the produced proof should be sent to `proposer(H + K, 0)`.

To generate the proof of block `H`, the prover needs the proof of the previous
block in strand `strand(H)`, whose height is `H - K`.
In the favorable scenario, `proof(H - K)` is included in block `H`, so the
production of `proof(H)` can start immediately.
Otherwise, the prover needs to compute `unproven(strand(H))` and follow the steps:

1. Go back to the block with the lowest height `Hmin` in
   `unproven(strand(H))`, which must include a non-empty `proof` field (by property 1.),
   and use `block(Hmin).proof` and `block(Hmin)` to produce `proof(Hmin)`;
   - Notice that in the favorable scenario `H == Hmin`, and the process is done here.
2. Go to the block `Hmin + K` and use `proof(Hmin)` and  `block(Hmin + K)` to
   produce `proof(Hmin + K)`. This operation should be faster because
   `block(Hmin + K)` must be empty (by property 2.).
3. If `Hmin + K == H`, the process is done. Otherwise, set `Hmin = Hmin + K`
   and repeat step 2.

At the end of the process, the prover has produced a single proof attesting the
proper execution of  **one full block**, at height `Hmin`,
and possibly, in the case of `|unproven(strand(H))| > 1`, also of the execution of
**some empty blocks**.
The produced proof is targeted to be included in the `proof` field of the block
proposed at height `H + K`.

### Proposers and Provers

The proofs produced by provers are expected to be included in committed blocks.
A committed block must have been produced by the **proposer** of a round of
consensus, possibly including a `proof` produced by a **prover**.

There is therefore a relation between provers and proposers.
The simpler way to define this relation is to assume that prover and proposer
are roles that are implemented by the same nodes.
So, once the block at height `H` is committed, the primary proposer of height
`H + K` starts producing `expected_proof(H + K)`.
If it is produced by the time height `H + K` starts, it is included in
the `proof` field of the block produced for that height.

We may consider, however, more complex setups where provers and proposers are
distinct nodes.
In this case, it has to be defined how the prover assigned to produce
`expected_proof(H + K)` interacts with the proposers of height `H + K`,
in particular with its primary proposer, the node defined by `proposer(H + K, 0)`.

The relation between provers and proposers is particularly relevant in the
scenario where multiple empty blocks, with empty `proof` fields, are
committed to a strand `s`.
Recall that there is a limit for the number of such blocks in a strand, at most
`P` can be produced.
So, if `|unproven(s)| > P`, then the proposer of **any round** of a height `H`,
where `strand(H) == s`, can only produce and propose a block if it includes
`expected_proof(H)`.

### Critical scenario

The critical scenario of the proofs scheduling happens when the system reaches
height `H* = H + (P + 1) * K` without having the proof of block `H` include in
any previous height in strand `s = strand(H) = strand(H*)`.
As [previously](#proposers-and-provers) mentioned, in this particular scenario,
a block can only be committed to height `H*` if it includes the proof of block
`H` combined with proofs for all empty blocks from heights 
`H' = H + i * K`, with `1 <= i <= P`.

In this scenario, two situations that are tolerated by proofs scheduling
protocol are not any longer accepted:

- The primary proposer `proposer(H*, 0)` is not allowed to propose an empty
  block if it has not produced, or retrieved from some prover,
  `expected_proof(H*)`, as correct validators will reject the proposed block;
- Non-primary proposers of height `H*` are not allowed to propose empty blocks
  either, for the same reason: correct validators will reject the
  proposed block, as it does not include the required `expected_proof(H*)`.

As a result, some prover must produce `expected_proof(H*)` and render it
available to the proposer of a round of height `H*` to be included in the
proposed block.
Until that happens, the blockchain will not progress, it is frozen. 
More precisely, the consensus algorithm will go through multiple rounds that
are all unsuccessful because the proposer cannot propose a block that the
validators would consider valid and therefore vote for.
Correct validator will instead prevote and precommit `nil`, and eventually go
to the next round, where the same happens.
In the case in which no prover has computed `expected_proof(H*)` by the
beginning of height `H*`, this height will require at least `L` time to produce
a block that can be committed, where `L` is probably in the order of minutes.

In order to prevent, or minimize as far as possible, this scenario, some
mechanisms should be considered in order to incentivize additional provers to
produce (redundant) proofs for unproven blocks, as long as some strand starts
to have multiple empty blocks.
As general principle, a such mechanism should allow to non-primary proposers of
heights belonging to a strand with several outstanding proofs to produce full
blocks, including the expected proof, even thought by the scheduling protocol
they are not _required_ to so before the above described critical height `H*`.

[starkprover]: https://docs.starknet.io/architecture-and-concepts/network-architecture/starknet-architecture-overview/#provers
