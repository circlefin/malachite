# Accountable Tendermint 

Accountable Tendermint is a slight variation of Tendermint that ensures the
detection of amnesia attacks in runs in which agreement is violated. Note that
this differs from double vote evidence. Double votes can be detected also in
(more likely) cases where agreement is not violated because only a few
individual processes misbehave. 

Still, this algorithm ensures that in all cases where agreement is violated, the
nodes collect evidence of who misbehaved, which is useful information for
forensics, etc. 

More formally, we are interested in the following property:

**[Accountability]**
- If there are less than `2f + 1` Byzantine faulty processes, out of a total of `3f + 1` processes 
- And agreement is violated (two correct processes decide differently), 
- Then a correct node should be able to collect sufficient data to identify at least `f + 1`
Byzantine processes.


## Why Tendermint does not have this property

We discuss [here](../misbehavior.md#what-cannot-be-done) that there are cases in
Tendermint where we cannot decide who misbehaved. The main
challenge is the following: If a process committed a value `v` in round `r`,
there are cases where we cannot say whether a prevote for a different value `v'`
in a larger round `r' > r` by the same process is according to the algorithm
(based on a proposal with a `validRound > r`) or not (which would be an amnesia 
attack).


## The algorithm

Accountable Tendermint consensus differs from the original Tendermint algorithm
just in the prevote message. In the original Tendermint, propose messages has
the fields (in Quint)
```
type Vote = {
        voteType: VoteType,     // prevote
        srcAddress: Address, 
        height: Height,
        round: Round,
        valueId: ValueId,
    }
```

Accountable Tendermint has the addition field `validRound` in Vote
```
type Vote = {
        voteType: VoteType,
        srcAddress: Address, 
        height: Height,
        round: Round,
        valueId: ValueId,
        validRound: Round, // only relevant if voteType == prevote
    }
```

The use of the additional field in the [pseudo code](./pseudo-code.md) of the
algorithm is as follows:
- line 22 and line 28: In these two rules, the prevote message that is sent now
  carries the valid round that is contained in the propose message that
  triggered the sending of the prevote message.
- line 36: an additional check is performed in the guard that the `validRound` field in the
  `2f + 1` prevotes is matching `validRound` of the proposal (`vr` in the pseudo code).

In the following we will use the following abbreviations
- `decide(r, v)`: a process decides in round `r` on value `v`
- `commit(r,v)`: a set of at least `2f + 1` precommit messages for round `r` on value `v`
- `polka(r,v)`: a set of at least `2f + 1` prevote messages for round `r` on value `v`


## Why Accountable Tendermint works

1. The first observation is that Accountable Tendermint works on certificates
  (polkas and commits) instead of individual messages.
2. A certificate contains votes from at least one correct process. This is
  because a certificate contains messages from at least `2f + 1` processes, and
  there are at most `2f` faulty processes under the assumption the
  [accountability](#accountability) property.
3. The accountability property only gives guarantees in case of disagreement,
  that is, if two correct processes decide on different values. By the decision
  rule of Tendermint (line 49), in order to decide a value `v` a process needs to
  see a commit for value `v`. That is, we have two commits for different values `v` and `v'`

    - If the two commits are from the same round, there is double vote evidence
    (the two commits contain double votes from at least `f + 1` faulty processes) 
    - Otherwise, there are two commits for different values and different
      rounds (the amnesia case).
        - Let `Commit(v,r)` be the commit for the smaller round. 
        - Let `r'` be the larger round of these two. By Point 2, one of the
          precommit messages are sent by a correct process
        - This process can only send the precommit in round `r'` if it has
          received a polka for this round. That is, when there is a `Polka(v',r',_)`
        - In the new algorithm, the prevote messages in the polka contain `validRound`
        - Again by Point 2, `Polka(v',r',_)` contains a message from a correct process
        - By careful case distinction (see below), one can show that from the
          existence of the two certificates `Commit(v,r)` and `Polka(v',r',_)` we
          can prove that there are two certificates `Commit(v,r)` and
          `Polka(v",r",_)` (observe that the polka need to be the one of round
          `r'`) that have the property that the intersection of the senders of
          the messages in the certificates 
            - contains only faulty processes
            - contains at least `f + 1` processes (by the size of the certificates)
        - this intersection of  at least `f + 1` faulty processes is evidence of an amnesia 
          attack

If we have the gossip assumption on certificates (if a correct process receives
a certificate `c` then every correct process will eventually deliver `c`), then
eventually every correct process receives the involved certificates and can
generate evidence.

### Detailed correctness argument

In [misbehavior.qnt](./misbehavior.qnt), we have specified two functions
  - `doublePrevotes`
  - `amnesiaVotes`

These functions take as input two certificates (two polkas or a commit and a
polka), check for conflicts, and return the votes of misbehaving nodes in the
case of conflicts. 


In the following, we discuss that in the case of disagreement,
the system generates certificates that can be used to produce evidence. If we
have the gossip property of certificates (if a correct process sees a
certificate then eventually every correct processes sees the same certificate),
this ensures that eventually all correct processes will produce evidence.


- In order to decide on a value `commitValue` in a round `commitRound`, a 
  correct process needs to see a
    - **Certificate 1:** `commit(commitRound, commitValue)`
- In order to see such a commit, a (potentially different) correct process must
  send a commit message for which it needs to see a 
    - **Certificate 2:** `polka(commitRound, commitValue, vr)`
- If two processes decide differently, then in addition to `decide(commitRound,
  commitValue)` we have `decide(r, v)` for `commitValue != v`. By the reasoning
of above, we also must have a 
    - **Certificate 3:** `polka(r, v, conflictRound)`
- if  `commitRound = r`, then there are two conflicting polkas 
 `polka(commitRound, commitValue, vr)` and `polka(commitRound, v, conflictRound)` (Certificate 2 and Certificate 3), which is evidence according to [doubleVotes](./misbehavior.qnt).
- otherwise, assume `commitRound < r`
    - we have 
        - `commit(commitRound, commitValue)` (Certificate 1) and 
        - `polka(r, v, conflictRound)` (Certificate 3),
    - and the following case distinction
        - `conflictRound < commitRound`. Certificate 1 and Certificate 3 are amnesia evidence according to [amnesiaVotes](./misbehavior.qnt).
        - `conflictRound = commitRound`. In order to have `polka(r, v, conflictRound)`, we need a 
            - **Certificate 4:** `polka(conflictRound, v, _)` 
            - Certificate 4 together with `polka(commitRound, commitValue, vr)` (Certificate 2) is evidence according to [doublePrevotes](./misbehavior.qnt).
        - `conflictRound > commitRound`: We are in the case of the Proposition below, that is, we have
            - `commit(commitRound, commitValue)` (Certificate 1),  
            - `polka(r, v, conflictRound)` (Certificate 3),
            - `r > commitRound`, and
            - `conflictRound > commitRound`.



**Proposition.** If there is a `commit(commitRound, commitValue)`, and a correct
processes `p` sends `⟨PREVOTE, r, v, conflictRound⟩` for `conflictRound > commitRound` 
and `r > commitRound`  and `v != commitValue`, then there
is evidence in the system.

1. Let `p` be the first correct processes who sends such a prevote for the
   smallest round (`r`).
2. It does so in Line 30 based on a `polka(conflictRound, v, vr)`, that is,
   `2f + 1` prevotes with these values
3. Observe that `vr <= commitRound`, as otherwise `p` would not be the first
   process who sends such a message. 
    - To see why this is the case, observe that `conflictRound > commitRound` and
     by line 28, we would have `conflictRound <= r`. 
    - If, by contradiction, we would have that `vr > commitRound`, the prevote in
     `polka(conflictRound, v, vr)` would satisfy the characterization of a
     prevote message of the statement of the proposition, so that the sender of
     this prevote message would be the first one instead of `p`; a contradiction. 
    - So we must have`vr <= commitRound`.
4. We now do a case distinction.  
5. `vr = commitRound`. By line 28, Process `p` sends the message on a 
   `Polka(vr, v, _)`. Observe that `vr = commitRound`, so we have `Polka(commitRound,
   v, _)`
    - By the assumption of the proposition we have a `commit(commitRound,
    commitValue)`. As every certificate contains a message by at least one
    correct process, and by line 36, there must be a `Polka(commitRound,
    commitValue, _)`
    - `Polka(commitRound, v, _)` and `Polka(commitRound, commitValue, _)`
    contain evidence in the form of conflicting prevotes from at least `f+1`
    processes. See function [doubleVotes](./misbehavior.qnt).
6. `vr < commitRound`. We have amnesia evidence (see function
   [amnesiaVotes](./misbehavior.qnt))
    - By the Proposition statement there is: `commit(commitRound, commitValue)` ,
    - by Line 2 there is: `polka(conflictRound, v, vr)`,
    - for `commitRound < conflictRound` and `vr < commitRound`
