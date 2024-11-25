// -*- mode: Bluespec; -*-

module disagreementRun {

import TendermintDSL.* from "../../TendermintDSL"
export TendermintDSL.*

const groupA : Set[Address]
val groupB = Correct.exclude(groupA)
val aList = groupA.fold(List(), (s, x) => s.append(x)) 
val bList = groupB.fold(List(), (s, x) => s.append(x)) 

// A run for too many faulty nodes

// TODO: assertions about proposer being faulty
run DisagreementRun = {
    val thisProposer = proposer(validatorSet, 0, 0)
    nondet valA = oneOf(Values)
    nondet valB = oneOf(Values.exclude(Set(valA)))
    init
    .then(2.reps(_ => ListTakeAStep(correctList)))
    .then(ListDeliverProposal(aList, mkProposal(thisProposer, 0, 0, valA, -1)))
    .then(ListDeliverProposal(bList, mkProposal(thisProposer, 0, 0, valB, -1)))
    .then(ListTakeAStep(correctList))
    // they voted differently
    .then(ListTakeAStep(correctList)) // consume step change
    .then(ListDeliverAllVotes(Prevote, aList.concat(faultyList), aList, validatorSet, 0, 0, Val(valA)))
    .then(ListDeliverAllVotes(Prevote, bList.concat(faultyList), bList, validatorSet, 0, 0, Val(valB)))
    .then(aList.concat(faultyList).length().reps(_ => ListTakeAStep(aList)))
    .then(bList.concat(faultyList).length().reps(_ => ListTakeAStep(bList)))
    .then(ListTakeAStep(correctList)) // timeout prevote started -> extra step to consume pending
    // they precommited differently
    .then(ListTakeAStep(correctList)) // consume step change
    .then(ListDeliverAllVotes(Precommit, aList.concat(faultyList), aList, validatorSet, 0, 0, Val(valA)))
    .then(ListDeliverAllVotes(Precommit, bList.concat(faultyList), bList, validatorSet, 0, 0, Val(valB)))
    .then(aList.concat(faultyList).length().reps(_ => ListTakeAStep(aList)))
    .then(bList.concat(faultyList).length().reps(_ => ListTakeAStep(bList)))
    .then(all{
        assert(not(AgreementInv)),
        unchangedAll
    })   
}

}
