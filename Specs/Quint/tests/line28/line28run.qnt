// -*- mode: Bluespec; -*-

module line28run {

import TendermintDSL.* from "./TendermintDSL"
export TendermintDSL.*

const otherSet: Set[Address]
val others = otherSet.fold(List(), (s, x) => s.append(x)) 

run runToLine28 =
    val nextProposer = proposer(validatorSet, 0, 1)
    init
    .then(all {
        // others should be at most 2/3.
        // if this assertion fails the set need to be set differently
        assert(3 * size(otherSet) <= 2 * size(validators)),  
        assert(3 * size(otherSet.union(Faulty)) > 2 * size(validators)),  
        everyoneReceivesProposal(correctList, validatorList, validatorSet, 0, 0, "blue")
    })
    // receive all prevotes    
    .then(fromPrevoteToPrecommit(correctList, correctList, validatorList, validatorSet, 0, 0, Val("blue")))
    // now the faulty nodes precommit nil
    .then(ListDeliverAllVotes(Precommit, faultyList, correctList, validatorSet, 0, 0, Nil))
    .then(faultyList.length().reps(_ => ListTakeAStep(correctList)))   
    // now the other precommits are delivered, so that timeoutPrecommit is started
    .then(ListDeliverAllVotes(Precommit, others, correctList, validatorSet, 0, 0, Val("blue")))
    .then(others.length().reps(_ => ListTakeAStep(correctList)))
    // TimeoutPrecommit is there an can fire and bring is to the next round.
    .then(all{
        assert(system.get(nextProposer).timeouts.contains((PrecommitTimeout, 0, 0))),
        everyoneReceivesProposalVR(correctList, validatorList, validatorSet, 0, 1, "blue", 0)
    })
    .then(all{
        assert(voteBuffer.get(nextProposer).contains(mkVote(Prevote, nextProposer, 0, 1, Val("blue")))),
        unchangedAll
    })

}