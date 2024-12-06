// -*- mode: Bluespec; -*-

module line28run {

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
    - uses the valid value stored to send proposal for valid value
    - correct processes take this to execute line 28 and prevote for this value
*/

run runToLine28 = 
    val nextProposer = proposer(validatorSet, 0, 1)
    nondet value = oneOf(Values)
    init
    // proposer sends proposal and correct processes receive -> send prevote value
    .then(all {
        // others should be at most 2/3.
        // if this assertion fails the set need to be set differently
        assert(3 * size(otherSet) <= 2 * size(validators)),  
        assert(3 * size(otherSet.union(Faulty)) > 2 * size(validators)), 
        // the following is required for fromPrevoteToPrecommit to have the right number of steps. 
        // We should better understand this to simplify test writing
        assert(2 * size(Faulty) + 1 == size(Correct)), 
        assert(not(Faulty.union(otherSet).contains(nextProposer))), 
        everyoneReceivesProposal(correctList, validatorList, validatorSet, 0, 0, value)
    })
    // receive all prevotes    
    .then(fromPrevoteToPrecommit(correctList, correctList, validatorList, validatorSet, 0, 0, Val(value)))
    // now the faulty nodes precommit nil
    .then(ListDeliverAllVotes(Precommit, faultyList, correctList, validatorSet, 0, 0, Nil))
    .then(faultyList.length().reps(_ => ListTakeAStep(correctList)))   
    // now the other precommits are delivered, so that timeoutPrecommit is started
    .then(ListDeliverAllVotes(Precommit, others, correctList, validatorSet, 0, 0, Val(value)))
    .then(others.length().reps(_ => ListTakeAStep(correctList)))
    // TimeoutPrecommit is there an can fire and bring is to the next round.
    .then(all{
        assert(system.get(nextProposer).timeouts.contains((PrecommitTimeout, 0, 0))),
        everyoneReceivesProposalVR(correctList, validatorList, validatorSet, 0, 1, value, 0)
    })
    .then(all{
        assert(voteBuffer.get(nextProposer).contains(mkVote(Prevote, nextProposer, 0, 1, Val(value)))),
        unchangedAll
    })


}
