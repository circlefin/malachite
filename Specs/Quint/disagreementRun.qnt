// -*- mode: Bluespec; -*-

module disagreementRun {

import TendermintDSL.* from "./TendermintDSL"
export TendermintDSL.*

const groupA : Set[Address_t]
val groupB = Correct.exclude(groupA)
val aList = groupA.fold(List(), (s, x) => s.append(x)) 
val bList = groupB.fold(List(), (s, x) => s.append(x)) 

// A run for too many faulty nodes

// TODO: assertions about proposer being faulty
run DisagreementRun = {
    val thisProposer = Proposer (validatorSet, 0, 0)
    init
    .then(2.reps(_ => ListTakeAStep(correctList)))
    .then(ListDeliverProposal(aList, { height: 0, proposal: "a", round: 0, src: thisProposer, validRound: -1 }))
    .then(ListDeliverProposal(bList, { height: 0, proposal: "b", round: 0, src: thisProposer, validRound: -1 }))
    .then(ListTakeAStep(correctList))
    // they voted differently
    .then(ListTakeAStep(correctList)) // consume step change
    .then(ListDeliverAllVotes("Prevote", aList.concat(faultyList), aList, validatorSet, 0, 0, "a"))
    .then(ListDeliverAllVotes("Prevote", bList.concat(faultyList), bList, validatorSet, 0, 0, "b"))  
    .then(aList.concat(faultyList).length().reps(_ => ListTakeAStep(aList)))
    .then(bList.concat(faultyList).length().reps(_ => ListTakeAStep(bList)))
    .then(ListTakeAStep(correctList)) // timeout prevote started -> extra step to consume pending
    // they precommited differently
    .then(ListTakeAStep(correctList)) // consume step change
    .then(ListDeliverAllVotes("Precommit", aList.concat(faultyList), aList, validatorSet, 0, 0, "a"))
    .then(ListDeliverAllVotes("Precommit", bList.concat(faultyList), bList, validatorSet, 0, 0, "b"))  
    .then(aList.concat(faultyList).length().reps(_ => ListTakeAStep(aList)))
    .then(bList.concat(faultyList).length().reps(_ => ListTakeAStep(bList)))
    .then(all{
        assert(not(AgreementInv)),
        unchangedAll
    })   
}

}
