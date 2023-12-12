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
    .then(reps(2, _ => valStep(slow))) // enter round 0 and consume step change
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
    .then(others.length().reps(_ => ListTakeAStep(others)))   
    // Others now go to next round on timeoutPrecommit
    .then(all{
        assert(SetFromList(others).forall(proc => system.get(proc).timeout.contains(("TimeoutPrecommit", 0, 0)))),
        everyoneReceivesProposal (others, validatorList, validatorSet, 0, 1, "green")
    })          
    .then(fromPrevoteToPrecommit (others, others, validatorList, validatorSet, 0, 1, "green"))
    .then(ListDeliverAllVotes("Precommit", others, others, validatorSet, 0, 1, "green"))
    .then(others.length().reps(_ => ListTakeAStep(correctList)))
    .then(all {
        assert(SetFromList(others).forall(proc => system.get(proc).es.chain.head() == "green")),
        unchangedAll
    })       
}

run DecideForFutureRoundRun = {
    val nextProposer = Proposer (validatorSet, 0, 1)
    AllButOneDecideInR1WhileSlowInR0
    .then(deliverProposal(slow,  { height: 0, proposal: "green", round: 1, src: nextProposer, validRound: -1 }))
    .then(valStep(slow))
    .then(ProcessDeliverAllVotes("Precommit", slow, others, validatorSet, 0, 1, "green"))
    .then(2.reps(_ => valStep(slow))) // TODO parameterize the 2 away
    .then(all { // slow skipped to round 1
        assert(system.get(slow).es.pendingStepChange == "propose" and system.get(slow).es.cs.round == 1),
        valStep(slow) // now it processes the step change and discovers the proposal -> it prevotes
    })
        .then(all { 
        assert(system.get(slow).es.pendingStepChange == "Prevote"),
        valStep(slow) // now it consumes the step change, but nothing happens
    })
    .then(valStep(slow)) // it receives the final precommit and decides
    .then(all {
        assert(validators.forall(proc => system.get(proc).es.chain.head() == "green")),
        unchangedAll
    })    
}

run DecideForFutureRoundPrecommitFirstRun = {
    val nextProposer = Proposer (validatorSet, 0, 1)
    AllButOneDecideInR1WhileSlowInR0
    .then(ProcessDeliverAllVotes("Precommit", slow, others, validatorSet, 0, 1, "green"))
    .then(2.reps(_ => valStep(slow))) // TODO parameterize the 2 away
    .then(all { // slow skipped to round 1
        assert(system.get(slow).es.pendingStepChange == "propose" and system.get(slow).es.cs.round == 1),
        valStep(slow) // now it processes the step change but nothing happens
    })
    .then(valStep(slow)) // it receives the final precommit
    .then(all { 
        assert(system.get(slow).timeout.contains(("TimeoutPrecommit", 0, 1))),
        deliverProposal(slow,  { height: 0, proposal: "green", round: 1, src: nextProposer, validRound: -1 })
    })
    .then(valStep(slow)) // receive proposal and decide
    .then(all {
        assert(validators.forall(proc => system.get(proc).es.chain.head() == "green")),
        unchangedAll
    })    
}

run RoundswitchRun = {
    AllButOneDecideInR1WhileSlowInR0
    .then(deliverVote(slow, { height: 0, id: "green", round: 1, src: "v1", step: "Precommit" }))
    .then(deliverVote(slow, { height: 0, id: "green", round: 1, src: "v2", step: "Prevote" })) //TODO parameterize to F
    .then(valStep(slow))
    .then(all {
        assert(system.get(slow).es.cs.round == 0),
        valStep(slow)})
    .then(all {
        assert(system.get(slow).es.cs.round == 1),
        unchangedAll
    })   
}   




}