// -*- mode: Bluespec; -*-

module someMultiRoundRuns {

import TendermintDSL.* from "../../TendermintDSL"
export TendermintDSL.*

const slow : Address
val otherSet = validators.exclude(Set(slow))
val others = otherSet.fold(List(), (s, x) => s.append(x)) 

run AllButOneDecideInR1WhileSlowInR0 = {
    val nextProposer = proposer(validatorSet, 0, 1)
    val thisProposer = proposer(validatorSet, 0, 0)
    init
    .then(reps(2, _ => valStep(slow))) // enter round 0 and consume step change
    .then(onlyProposerReceivesProposal(others, validatorList, validatorSet, 0, 0, "blue"))

    // receive all prevotes    
    .then(ListDeliverAllVotes(Prevote, List(thisProposer), others, validatorSet, 0, 0, Val("blue")))
    .then(ListDeliverAllVotes(Prevote, others.select(x => x != thisProposer), others, validatorSet, 0, 0, Nil)) 
    .then(others.length().reps(_ => ListTakeAStep(others)))   
    .then(all{
        assert(SetFromList(others).forall(proc => system.get(proc).timeouts.contains((PrevoteTimeout, 0, 0)))),
        ListTimesOut(others)
    })
    .then(all{ 
        assert(SetFromList(others).forall(proc => system.get(proc).es.pendingStepChange == PrecommitStep)),
        ListTakeAStep(others)
    })
    .then(ListDeliverAllVotes(Precommit, others, others, validatorSet, 0, 0, Nil))
    .then(others.length().reps(_ => ListTakeAStep(others)))   
    // Others now go to next round on timeoutPrecommit
    .then(all{
        assert(SetFromList(others).forall(proc => system.get(proc).timeouts.contains((PrecommitTimeout, 0, 0)))),
        everyoneReceivesProposalTO (others, validatorList, validatorSet, 0, 1, "green")
    })          
    .then(fromPrevoteToPrecommit(others, others, validatorList, validatorSet, 0, 1, Val("green")))
    .then(ListDeliverAllVotes(Precommit, others, others, validatorSet, 0, 1, Val("green")))
    .then(others.length().reps(_ => ListTakeAStep(correctList)))
    .then(all {
        assert(SetFromList(others).forall(proc => system.get(proc).es.chain.head().decision.proposal == "green")),
        unchangedAll
    })       
}

run DecideForFutureRoundRun = {
    val nextProposer = proposer(validatorSet, 0, 1)
    AllButOneDecideInR1WhileSlowInR0
    .then(deliverProposal(slow, mkProposal(nextProposer, 0, 1, "green", -1)))
    .then(valStep(slow))
    .then(ProcessDeliverAllVotes(Precommit, slow, others, validatorSet, 0, 1, Val("green")))
    .then(others.length().reps(_ => valStep(slow))) 
    .then(all {
        assert(validators.forall(proc => system.get(proc).es.chain.head().decision.proposal == "green")),
        unchangedAll
    })    
}

run DecideForFutureRoundPrecommitFirstRun = {
    val nextProposer = proposer(validatorSet, 0, 1)
    AllButOneDecideInR1WhileSlowInR0
    .then(ProcessDeliverAllVotes(Precommit, slow, others, validatorSet, 0, 1, Val("green")))
    .then(2.reps(_ => valStep(slow))) // TODO parameterize the 2 away
    .then(all { // slow skipped to round 1
        assert(system.get(slow).es.cs.round == 1),
        valStep(slow) // now it processes the step change but nothing happens
    })
    .then(valStep(slow)) // it receives the final precommit
    .then(all { 
        assert(system.get(slow).timeouts.contains((PrecommitTimeout, 0, 1))),
        deliverProposal(slow, mkProposal(nextProposer, 0, 1, "green", -1))
    })
    .then(valStep(slow)) // receive proposal and decide
    .then(all {
        assert(validators.forall(proc => system.get(proc).es.chain.head().decision.proposal == "green")),
        unchangedAll
    })    
}

run RoundswitchRun = {
    AllButOneDecideInR1WhileSlowInR0
    .then(deliverVote(slow, mkVote(Precommit, "v1", 0, 1, Val("green"))))
    .then(deliverVote(slow, mkVote(Prevote, "v2", 0, 1, Val("green")))) //TODO parameterize to F
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
