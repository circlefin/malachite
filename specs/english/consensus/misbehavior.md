# Misbehavior detection and handling

TODO Generate the sense for the problem

Copy from below:
While a single instance of an unintentional double vote of one validator does
not pose big problems (it cannot bring disagreement), repeated unintentional
double votes by several validator operators having large voting power might
eventually lead to disagreement and a chain halt. Therefore it make sense to
incentivize individual operators to fix their setup while the whole system is
still operational.

## Background

### Misbehavior types

Tendermint is a variant of the [seminal
algorithm](https://groups.csail.mit.edu/tds/papers/Lynch/MIT-LCS-TM-270.pdf) by
Dwork, Lynch and Stockmeyer. It shares the property that if less than a third of
the validators are faulty, agreement is guaranteed. If there are more than two
thirds of faulty validators, they have control over the system.

In order to bring the system to disagreement, the faulty validators need to
actively deviate from the [protocol](TODO link to Daniel's pseudo code). By
superficial inspection of the pseudo code we observe that 

- **[Double vote]** correct validators never send two (different) vote messages
  (prevote, precommit) for the same height and round, and
- **[Double propose]** a correct proposer never send two different proposals for
  the same height and round, and
- **[Bad proposer]** a correct validator whose ID is different from
  `proposer(h, round)`  does not send a proposal for that height and round.

A little bit more involved inspection shows that if a correct processes locks a
value (setting `lockedValue` and `lockedRound` in lines 38 and 39) then it sends
a prevote for a different value in a later round (line 30) **only if** the
condition of lines 28/29 is satisfied, that is, only of it receives a proposal
and 2f+1 matching prevotes that carry the value `vr` that satisfies `vr >=
lockedRound` (line 29). In other words

- **[Amnesia]** a correct validators never sends a prevote for a value `val` if
  it has locked a different value `val2` before and hasn't received a proposal
  and sufficiently many  prevotes for `val2` with `vr >= lockedRound`.

Remark on the term "amnesia". Amnesia a violation of the locking mechanism
introduced by Dwork, Lynch, and Stockmeyer into their algorithm: a process locks
a value in a round if the value is supported by more than 2/3. A process that
has locked a value can only be convinced to release that lock if more than two
thirds of the processes have a lock for a later round. In the case of less than
a third faults, if a process decides value v in a round r the algorithm ensures
that more than two thirds have a lock on value v for that round. As a result
once a value is decided, no other value w will be supported by enough correct
processes. However, if there are more than a third faults, adversarial processes
may lock a value v and in a later round “forget” they did that and support a
different value.

It has been shown by formal verification (see results obtained with
[Ivy](https://github.com/cometbft/cometbft/tree/main/spec/ivy-proofs), and
[Apalache](https://github.com/cometbft/cometbft/blob/main/spec/light-client/accountability/Synopsis.md))
that if there are between one third and two thirds of faults, every attack on
Tendermint consensus that leads to violation of agreement is either the a
"double vote" or an "amnesia attack". 

### Accountability

The question we are interested is, while we cannot prevent disagreement in all
cases, wether we can keep misbehaving nodes accountable by ensuring to collect
evidence of misbehavior, either for online evidence handling (e.g., penalties),
or in case of a forking event, forensic analysis of the attack scenario that can
constitute a source of information for social or legal actions after-the-fact.

CometBFT only record specific misbehavior, namely the [duplicate vote
evidence](https://github.com/cometbft/cometbft/blob/main/spec/core/data_structures.md#duplicatevoteevidence).
While attacks are rare, such behavior has been observed as a result of
misconfiguration. Most companies operating a validator typically implement this
node as a fault-tolerant setup itself, having copies of the private key of the
validator on multiple machines. If such a fault-tolerant setup is implemented
poorly or misconfigured, this may result in duplicate (and sometimes
conflicting) signatures in a protocol step, although no actual attack was
intended. Still, such behavior may be used for mild penalties (e.g., not paying
fees to the validator for some time, taking a small penalty of their stake), as
part of the incentivization scheme motivating validator operators to fix such
issues and ensure reliability of their node. 

While a single instance of an unintentional double vote of one validator does
not pose big problems (it cannot bring disagreement), repeated unintentional
double votes by several validator operators having large voting power might
eventually lead to disagreement and a chain halt. Therefore it make sense to
incentivize individual operators to fix their setup while the whole system is
still operational.

 
## Misbehavior detection and verification

### What can be done based on Tendermint consensus

#### Double vote

- Detection: One needs to observe two different vote messages signed by the same validator
for the same
    - step (prevote, precomit)
    - round
    - height
    - chainID (this is relevant in the context resetting to previous heights or multiple chains)

We observe that the verification data is very minimal. We do not need any application-level data, and can even use it to convince an outside observer that the node misbehaved.

#### Double propose

Similar to double vote/

#### Bad proposer

- Detection: One needs to observe 
    - a propose message for
        - round
        - height
        - chainID
    - knowledge of the `proposer(h, round)` function and the context in which it is run.   

Observe that the way it is typically implemented, `proposer(h, round)` is not a "mathematical function" that takes as input the height and the round and produces an ID. Rather it is typically implemented as a stateful function that is based on priorities. The latter depend on voting powers and who has been proposer in previous heights.

Verification is more complex than double vote and double propose:

- In contrast to double vote, where it is still trivial to verify the misbehavior evidence a week after it was generated, in order to verify bad proposer we need knowledge on the validator priorities at that time. 
- multiple layers are involved
    - maintaining and updating voting powers is typically an application level concern
    - the `proposer` function is situated at the consensus level
    - misbehavior detection can only happen and consensus level
    - in order to use the evidence, the application must be able to verify the evidence. This this case it means that the application must
        - be aware of the consensus-level `proposer` function and priorities
        - potentially have historical data (the evidence might come a couple of blocks after the fact) on validator sets

#### Amnesia





 