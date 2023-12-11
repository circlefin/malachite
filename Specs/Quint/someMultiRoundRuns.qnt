// -*- mode: Bluespec; -*-

module someMultiRoundRuns {

import TendermintDSL.* from "./TendermintDSL"
export TendermintDSL.*

const slow : Address_t
val otherSet = validators.exclude(Set(slow))
val others = otherSet.fold(List(), (s, x) => s.append(x)) 

run AllButOneDecideInR1WhileSlowInR0 = {
    val nextProposer = Proposer (validatorSet, 0, 1)
    val thisProposer = Proposer (validatorSet, 0, 0)
    init
    .then(onlyProposerReceivesProposal(others, validatorList, validatorSet, 0, 0, "blue"))

    // receive all prevotes    
    .then(ListDeliverAllVotes("Prevote", List(thisProposer), others, validatorSet, 0, 0, "blue"))
    .then(ListDeliverAllVotes("Prevote", others.select(x => x != thisProposer), others, validatorSet, 0, 0, "nil")) 
    .then(others.length().reps(_ => ListTakeAStep(others)))   
    .then(all{
        assert(SetFromList(others).forall(proc => system.get(proc).timeout.contains(("TimeoutPrevote", 0, 0)))),
        ListTakeAStep(others)
    })
    .then(all{ 
        assert(SetFromList(others).forall(proc => system.get(proc).es.pendingStepChange == "Precommit")),
        ListTakeAStep(others)
    })
    .then( ListDeliverAllVotes ("Precommit", others, others, validatorSet, 0, 0, "nil")) 
            
    // // now the faulty nodes precommit nil
    // .then(ListDeliverAllVotes("Precommit", faultyList, correctList, validatorSet, 0, 0, "nil"))
    // .then(faultyList.length().reps(_ => ListTakeAStep(correctList)))   
    // // now the other precommits are delivered, so that timeoutPrecommit is started
    // .then(ListDeliverAllVotes("Precommit", others, correctList, validatorSet, 0, 0, "blue"))
    // .then(others.length().reps(_ => ListTakeAStep(correctList)))
    // // TimeoutPrecommit is there an can fire and bring is to the next round.
    // .then(all{
    //     assert(system.get(nextProposer).timeout.contains(("TimeoutPrecommit", 0, 0))),
    //     everyoneReceivesProposal(correctList, validatorList, validatorSet, 0, 0, "red")
    // })
    // .then(all{
    //     assert(voteBuffer.get(nextProposer).contains( { height: 0, id: "blue", round: 1, src: nextProposer, step: "Prevote" })),
    //     unchangedAll
    // })
}

}