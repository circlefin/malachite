// -*- mode: Bluespec; -*-

module line26run {

import TendermintDSL.* from "../../TendermintDSL"
export TendermintDSL.*

const otherSet : Set[Address]
val others = otherSet.fold(List(), (s, x) => s.append(x)) 

/*
- there need to be faulty processes
- there are other processes that together with the faulty ones are 2f+1
- the others include the proposer.
- the others are successful until sending precommit value
    - they set locked value and valid value
- then faulty precommit nil are delivered to correct processes
- the precommits from others are delivered to correct processes so that timeoutprecommit is started
- correct processes go to the next round
- there the proposer
    - ignores the valid value stored and send a proposal for a different value
    - correct processes take this to execute line 26 and prevote nil
*/

run runToLine26 = {
    val nextProposer = proposer(validatorSet, 0, 1)
    nondet value = oneOf(Values)
    nondet otherValue = oneOf(Values.exclude(Set(value)))
    init

    // proposer sends proposal and correct processes receive -> send prevote value
    .then(all {
        // others should be at most 2/3.
        // if this assertion fails the set need to be set differently
        assert(3 * size(otherSet) <= 2 * size(validators)),  
        assert(3 * size(otherSet.union(Faulty)) > 2 * size(validators)),  
        everyoneReceivesProposal(correctList, validatorList, validatorSet, 0, 0, value)
    })
    // correct processes receive prevotes from correct processes -> send precommit    
    .then(fromPrevoteToPrecommit(correctList, correctList, validatorList, validatorSet, 0, 0, Val(value)))
    // TODO assertion: valid and locked is set.
    .expect(Correct.forall(p => system.get(p).es.cs.lockedValue == Val(value)))
    // now the faulty nodes precommit nil
    .then(ListDeliverAllVotes(Precommit, faultyList, correctList, validatorSet, 0, 0, Nil))
    // correct processes receive faulty precommit nil
    .then(faultyList.length().reps(_ => ListTakeAStep(correctList)))   
    // now the precommits from some correct processes are delivered, 
    // "other" processes are just enough so that timeoutPrecommit is started
    .then(ListDeliverAllVotes(Precommit, others, correctList, validatorSet, 0, 0, Val(value)))
    .then(others.length().reps(_ => ListTakeAStep(correctList)))
    // now fire precommit timeout:
    .then(ListTakeAStep(correctList))
    .then(ListTimesOut(correctList))
    // correct process deliver proposal
    .then(ListDeliverProposal(correctList, mkProposal(nextProposer, 0, 1, otherValue, -1)))
    // proper consume round switch + process proposal (2 steps)
    .then(2.reps(_ => ListTakeAStep(correctList)))
    // now line26 made us prevote nil in round 1
    .then(all{
	    assert(voteBuffer.get("v1").contains(mkVote(Prevote, "v4", 0, 1, Nil))),
        unchangedAll
    })
}

}
