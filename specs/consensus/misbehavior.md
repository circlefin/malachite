# Misbehavior detection and handling

This documents describes different kinds of misbehavior by faulty processes
that over time can harm the system (lead to disagreement), and how each
misbehavior is defined and can be detected.


## Background

### Misbehavior types

Tendermint is a variant of the [seminal
algorithm](https://groups.csail.mit.edu/tds/papers/Lynch/MIT-LCS-TM-270.pdf) by
Dwork, Lynch and Stockmeyer. It shares the property that if less than a third of
the processes are faulty, agreement is guaranteed. If there are more than two
thirds of faulty processes, they have control over the system.

In order to bring the system to disagreement, the faulty processes need to
actively deviate from the [protocol](./pseudo-code.md). By
superficial inspection of the pseudo code we observe that 

- **[Double vote]** correct processeses never send two (different) vote messages
  (`PREVOTE`, `PRECOMMIT`) for the same height and round (that is the messages
  differ in the value they carry; also `nil` is considered a value here), and
- **[Double propose]** a correct proposer never send two different proposals for
  the same height and round, and
- **[Bad proposer]** a correct processes whose ID is different from the one
  returned by `proposer(h, r)`  does not send a proposal for height `h` and 
  round `r `.

A little bit more involved inspection shows that if a correct process `p` locks a
value (setting `lockedValue_p` and `lockedRound_p` in lines 38 and 39) then it sends
a prevote for a different value in a later round (line 30) **only if** the
condition of lines 28/29 is satisfied, that is, only of it receives a proposal
and 2f+1 matching prevotes for the value in round `vr` that satisfies `vr >=
lockedRound_p` (line 29). In other words

- **[Amnesia]** a correct process never sends a prevote for a value `val` if
  it has locked a different value `val2` before and hasn't received a proposal
  and sufficiently many prevotes for `val2` with valid round `vr >= lockedRound_p`.

Remark on the term "amnesia". Amnesia a violation of the locking mechanism
introduced by Dwork, Lynch, and Stockmeyer into their algorithm: a process locks
a value in a round if the value is supported by more than 2/3 of the processes. A process that
has locked a value can only be convinced to release that lock if more than two
thirds of the processes have a lock for a later round. In the case of less than
a third faults, if a process decides value `v` in a round `r` the algorithm ensures
that more than two thirds have a lock on value `v` for that round. As a result
once a value is decided, no other value `v' != v` will be supported by enough correct
processes. However, if there are more than a third faults, adversarial processes
may lock a value `v` and in a later round "forget" they did that and support a
different value.

It has been shown by formal verification (see results obtained with
[Ivy](https://github.com/cometbft/cometbft/tree/main/spec/ivy-proofs), and
[Apalache](https://github.com/cometbft/cometbft/blob/main/spec/light-client/accountability/Synopsis.md))
that if there are between one third and two thirds of faults, every attack on
Tendermint consensus that leads to violation of agreement is either a
"double vote" equivocation or an "amnesia attack". 

### Accountability

The question we are interested is, while we cannot prevent disagreement in all
cases, whether we can keep misbehaving nodes accountable by ensuring to collect
evidence of misbehavior, either for online evidence handling (e.g., penalties),
or in case of a forking event, forensic analysis of the attack scenario that can
constitute a source of information for social or legal actions after-the-fact.

CometBFT only record specific misbehavior, namely the [duplicate vote
evidence](https://github.com/cometbft/cometbft/blob/main/spec/core/data_structures.md#duplicatevoteevidence).
While attacks are rare, such behavior has been observed as a result of
misconfiguration. Most companies operating a consensus process (also known as a
_validator_) typically implement this
node as a fault-tolerant setup itself, having copies of the private key of the
process on multiple machines. If such a fault-tolerant setup is implemented
poorly or misconfigured, this may result in duplicate (and sometimes
conflicting) signatures in a protocol step, although no actual attack was
intended. Still, such behavior may be used for mild penalties (e.g., not paying
fees to the validator for some time, taking a small penalty of their stake), as
part of the incentivization scheme motivating operators to fix such
issues and ensure reliability of their node. 

While a single instance of an unintentional double vote of one process does
not pose big problems (it cannot bring disagreement), repeated unintentional
double votes by several processes having large voting power might
eventually lead to disagreement and a chain halt. Therefore it make sense to
incentivize individual operators to fix their setup while the whole system is
still operational.

 
## Misbehavior detection and verification based on Tendermint consensus

### What can be done 

#### Double vote

- Detection: One needs to observe two different vote messages signed by the same
process for the same
    - round step (`prevote` or `precomit`)
    - round
    - height
    - chainID (this is relevant in the context resetting to previous heights or
      multiple chains)

We observe that the verification data is very minimal. We do not need any
application-level data, and can even use it to convince an outside observer that
the node misbehaved.

#### Double propose

Similar to double vote. Observe that in the implementation there is a difference between
a small proposal message carrying only the has of the value, and the big proposal with all the data that comes in parts.

#### Bad proposer

- Detection: One needs to observe 
    - a `PROPOSAL` message for
        - round `r`
        - height `h`
        - chainID
    - knowledge of the `proposer(h, r)` function and the context in which it
      is run.   

Observe that the way it is typically implemented, `proposer(h, r)` is not a
"mathematical function" that takes as input the height, the round, and the set
of processes running that height and returns a process.
Rather it is typically implemented as a function that keeps an internal state,
for instance to represent the process priorities, as in the case of
the [proposer selection procedure of CometBFT][cometbft-proposer].
The latter depends on voting powers and who has been proposer in previous
heights.

Verification is more complex than double vote and double propose:

- In contrast to double vote, where it is still trivial to verify the
  misbehavior evidence a week after it was generated, in order to verify bad
  proposer we may need knowledge of the internal state of the proposer selection
  algorithm at that time.
- multiple layers are involved
    - maintaining and updating voting powers is typically an application level
      concern
    - the [`proposer(h, r)` function](./overview.md#proposer-selection) is
      situated at the consensus level
    - misbehavior detection can only happen at consensus level
    - in order to use the evidence, the application must be able to verify the
      evidence. This this case it means that the application must
        - be aware of the consensus-level `proposer(h, r)` function and its
          internal state, namely, be able to reproduce the output of
          `proposer(h, r)` for any given height
        - potentially have historical data (the evidence might come a couple of
          blocks after the fact) on the set of processes running consensus at
          multiple heights

### What cannot be done

#### Amnesia

Let's consider the following case, we have received the following signed message
from process `p`

- `⟨PRECOMMIT, h, 0, id(v))`.

By code inspection, we understand that `p` has locked value `v` in round `0`.
Now assume we receive any of the following messages signed by `p`. 

- `(PROPOSAL, h, 2, id(v'), 1)`
- `(PREVOTE, h, 2, id(v'))`
- `(PRECOMMIT, h, 2, id(v'))`

The question is, did `p` misbehave? Let's consider some cases

**Case 1.** There are at most f faulty processes and process `p` is the only
one who locked or updated its valid value in round 0. 

- Then a correct proposer of round 1 will propose a different value `v'`, 
- 2f+1 correct processes will vote for `v'` in round 1 (`p` cannot because it is locked)
- There are some faulty prevote nil that are received the prevote from the correct processes
- so that all process run into timeoutPrevote
- after that all correct processes will get all the prevotes for `v'` and will update validValue
- assume in round 2, `p` is the proposer
    - it will send `(PROPOSAL, h, 2, id(v'), 1)` (although it still has a lock on `v`)
    - in the lucky path all correct processes, including `p` will send 
        - `(PREVOTE, h, 2, id(v'))`, and later
        - `(PRECOMMIT, h, 2, id(v'))`
        
**Case 2.** There are at most f faulty processes and all correct processes lock
and updated their valid value in round 0. As discussed in the background section, the
algorithm is designed in a way that no correct process will ever send any
message for a value different from `v`. 

So after sending `⟨precommit, h, 0, id(v))`, process `p`:

- in runs of Case 1 is allowed (even forced) to also send these three messages, while
- in runs of Case 2 it would be misbehaving.

So the pair (`⟨PRECOMMIT, h, 0, id(v))`, `(PREVOTE, h, 2, id(v'))`), or the pairs with a proposal or a precommit for `v'`, do not constitute misbehavior. 


## Misbehavior detection and verification in Accountable Tendermint

The extended version of this
[paper](https://infoscience.epfl.ch/server/api/core/bitstreams/bb494e9a-22aa-43a2-b995-69c7a2cc893e/content)
proposes a slight change to the Tendermint algorithm that allows us to achieve
the following property

**Accountability.** If there are at most `2f` Byzantine-faulty processes and (at least) two
correct processes decide different values, then every correct process eventually
detects at least `f + 1` faulty processes.

The change to Tendermint is just that prevote messages have an additional field
that carries the content of the `vr` field of the proposal that triggered
the sending of the prevote. 
See [Accountable Tendermint](./accountable-tm/README.md) for more details.

[cometbft-proposer]: https://github.com/cometbft/cometbft/blob/main/spec/consensus/proposer-selection.md
