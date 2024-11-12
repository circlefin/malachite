# Accountability


It has been decided to build de-centralized Starknet using consensus engines that are based on the well-known [Tendermint consensus](https://arxiv.org/abs/1807.04938) algorithm. To be safe and live, Tendermint consensus requires that more than 2/3 of the participants are correct, that is, follow the algorithm. In a Proof-of-Stake (PoS) context, setting up a correct node is a technological challenge itself, e.g., (1) the node needs to have its private key it uses to sign consensus messages on a computer that is continuously connected to the Internet, which poses a security challenge to the setup or (2) the node needs to have high availability, as downtime of one node may results in downtime (or reduced performance, e.g., throughput) of the whole chain. We thus argue in the following that it is best practice to incentivize node operators to take up this technological challenge seriously.

A prerequisite to such incentivization schemes is to collect evidence of misconfiguration or misbehavior. When we talk about evidence here, we are only interested in provable pieces of information, e.g., if a node has used its private key to sign to conflicting messages (which is forbidden by PoS consensus algorithms, including Tendermint), that is, so-called equivocation (double vote). We don't consider subjective criteria, e.g., whether a node did not respond before a timeout expired.

**What is the typical case for equivocation seen in production systems?** Let's look at
[CometBFT](https://github.com/cometbft/cometbft). CometBFT is a battle-tested consensus engine based on Tendermint consensus, which
only records specific misbehavior, namely the duplicate vote evidence. While actual attacks are rare, equivocation has still been observed in production as a result of misconfiguration. Many companies operating a validator typically implement this node as a fault-tolerant setup itself (in order to achieve availability), having copies of the private key of the validator on multiple machines. For instance, the two tools [tmkms](https://github.com/iqlusioninc/tmkms) and [Horcrux](https://github.com/strangelove-ventures/horcrux) help managing validator keys.
If, however, a fault-tolerant setup would be implemented poorly or misconfigured, this may result in duplicate (and sometimes conflicting) signatures in a protocol step, although no actual attack was intended.

While a single instance of an unintentional double vote of one process typically does not pose big problems (it cannot bring disagreement), **repeated unintentional double votes by several validators having large voting power might eventually lead to disagreement** and a chain halt. Therefore it make sense to incentivize individual operators to fix their setup while the whole system is still operational.

Thus we propose that also in Starknet such behavior should lead to mild penalties (e.g., not paying fees to the validator for some time, taking a small portion of their stake as penalty), as part of the incentivization scheme motivating validator operators to fix such issues and ensure reliability of their node. I think the concrete incentivization scheme is a matter for the Starknet community and the node operators to agree on; all this lies in the application layer. In the remainder of this post, I would like to focus on the consensus layer, and lay out some options regarding what provable evidence consensus may provide to the application.

## Misbehavior types

Here we give some explanation about attacks on Tendermint. If you are aware of those, and are just interested in our conclusions, just scroll down to the [last section](#what-evidence-to-collect).

Tendermint is a variant of the [seminal DLS
algorithm](https://groups.csail.mit.edu/tds/papers/Lynch/MIT-LCS-TM-270.pdf) by
Dwork, Lynch and Stockmeyer. It shares with DLS the property that if less than one third of
the processes are faulty, agreement is guaranteed. If there are more than two
thirds of faulty processes, they have control over the system.

In order to bring the system to disagreement, the faulty processes need to
actively deviate from the protocol. By
superficial inspection of the pseudo code (cf. Algorithm 1 in the 
[arXiv paper](https://arxiv.org/abs/1807.04938)), we derive the 
following:

- **[Double vote]** correct processeses never send two (conflicting) vote messages
  (`PREVOTE`, `PRECOMMIT`) for the same height and round (that is the messages
  differ in the value they carry; also `nil` is considered a value here), and
- **[Double propose]** a correct proposer never send two different proposals (i.e., `PROPOSAL` messages) for
  the same height and round, and
- **[Bad proposer]** a correct processes whose ID is different from the one
  returned by `proposer(h, r)`  does not send a proposal for height `h` and 
  round `r`.

A more involved inspection shows that if a correct process `p` locks a
value (setting `lockedValue_p` and `lockedRound_p` in lines 38 and 39) then it sends
a prevote for a different value in a later round (line 30) **only if** the
condition of lines 28/29 is satisfied. That is, only of it receives a proposal
and 2f+1 matching prevotes for the value in round `vr` that satisfies `vr >=
lockedRound_p` (line 29). In other words

- **[Amnesia]** a correct process never sends a prevote for a value `v` if
  it has locked a different value `v'` before and hasn't received a proposal
  and sufficiently many prevotes for `v'` with valid round `vr >= lockedRound_p`.

Remark on the term "amnesia". Amnesia a violation of the locking mechanism
introduced by Dwork, Lynch, and Stockmeyer into their algorithm: a process locks
a value in a round if the value is supported by more than 2/3 of the processes. A process that
has locked a value can only be convinced to release that lock if more than two
thirds of the processes have a lock for a later round. In the case of less than
one third faults, if a process decides value `v` in a round `r` the algorithm ensures
that more than two thirds have a lock on value `v` for that round. As a result
once a value is decided, no other value `v' != v` will be supported by enough correct
processes. However, if there are more than one third faults, adversarial processes
may lock a value `v` in a round and in a later round "forget" they did that and support a
different value.

It has been shown by formal verification (see results obtained with
[Ivy](https://github.com/cometbft/cometbft/tree/main/spec/ivy-proofs), and
[Apalache](https://github.com/cometbft/cometbft/blob/main/spec/light-client/accountability/Synopsis.md))
that if there are between one third and two thirds of faults, every attack on
Tendermint consensus that leads to violation of agreement is either a
"double vote" or an "amnesia" attack. 

## What evidence to collect

We argue that the only two types of evidence that make sense to collect are "double vote" and "amnesia". By the verification results mentioned above, they are the ones actually required to disrupt the system. 

### Why not "double propose"?

First, it doesn't harm safety by itself, as processes also need to double vote to produce agreement violations.
Second, in consensus engine implementations, sometimes there are no self-contained `PROPOSAL` messages, but rather they are big chunks of data that is transmitted in block parts or streamed, so that the mapping of algorithmic `PROPOSAL` messages to what we see in implementations is not so direct. Consequently, we don't think it makes sense to go down this rabbit hole.

### Why not "bad proposer"?

First, by itself it doesn't harm safety, as correct processes will just disregard the produced proposals. 
Second, we are only interested in "provable evidence". So while in principle it can be proven, much more data, partly on consensus internals, needs to be included in the evidence. Checking that a process was not the proposer of a certain round and height requires knowing the state of the proposer selection algorithm at this specific point. Which is implemented and depends on the state of the application at that point. Again, it doesn't seem to make sense to investigate this, given that there is no value added.

### Why "double vote"?

We have laid out above that just to keep the system stable and operational, an incentivization scheme against double votes is very pragmatic. It motivates validator operators to fix misconfigurations and ensure reliability of their nodes.
So it makes sense that the consensus engine collects this. Observe that in contrast to "bad proposer" discussed above, the data to prove misbehavior is very concise. See the [evidence data structure](https://github.com/cometbft/cometbft/blob/main/spec/core/data_structures.md#duplicatevoteevidence) from CometBFT, which basically just consists of two signed vote messages.

### What about Amnesia?

Regarding the amnesia attack, there are trade-offs that we would like to start a discussion around:

- Pros
  - together with "double vote" this would allow an incentivization scheme against all behaviors that can lead to disagreement
  - it would allow us to shield the consensus engine against all attacks on safety, since we could generate evidence for forensics
- Cons
  - out-of-the-box, Tendermint consensus does not support provable amnesia evidence. However, we have developed a slight adaptations of Tendermint (roughly speaking, it adds one additional round field to votes), that would make amnesia provable. (It doesn't involve extra steps or performance penalties, but this is actually a Pro)
  - our solution doesn't necessarily help with the "fix misconfigurations" issue as it only produces evidence when we have conflicting commits 

## Conclusions

We argue that a mild form of incentivization is useful to stabilize the system and keep it operational. Such incentivization scheme must be based on provable data. Based on these two requirements we suggest that the consensus engines may collect two types of evidence. We strongly are in favor of "double vote" evidence and recommend to the Starknet community to agree on an incentivization scheme that is acceptable for users and node operators. We are also in favor of considering "amnesia" evidence, although this perhaps needs a broader discussion.