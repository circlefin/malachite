// -*- mode: Bluespec; -*-

module decideForPastRound {

import TendermintDSL.* from "../../specs/TendermintDSL"
export TendermintDSL.*

const testedVal : Address
const otherSet : Set[Address]
val others = otherSet.fold(List(), (s, x) => s.append(x)) 

/*
- there is a correct validator "testedVal" that we want to observe
- there need to be faulty processes
- there are *other* processes that together with the faulty ones are 2f+1
- the others include the proposer.
- the faulty processes help the others to decide in round 0
- testedVal goes to round 1 upon timeoutPrecommit

*/

run runToDecision = {
    val nextProposer = proposer(validatorSet, 0, 1)
    init
    .then(valStep(testedVal))
// proposer sends proposal and other processes receive it -> send prevote value
    .then(all {
        // others should be at most 2/3.
        // if this assertion fails the set need to be set differently
        assert(3 * size(otherSet) <= 2 * size(validators)),  
        assert(3 * size(otherSet.union(Faulty)) > 2 * size(validators)),  
        everyoneReceivesProposal(others, validatorList, validatorSet, 0, 0, "blue")
    })

// Others receive the prevotes of others and the faulty ones -> send precommit value
    .then(ListDeliverAllVotes (Prevote, others.concat(faultyList), others, validatorSet, 0, 0, Val("blue")))
    .then(others.concat(faultyList).length().reps(_ => ListTakeAStep(others)))
    .then(ListTakeAStep(others))

// Precommit from faulty processes are delivered -> value to others, nil to testedVal
    .then(ListDeliverAllVotes (Precommit, faultyList, others, validatorSet, 0, 0, Val("blue")))
    .then(ProcessDeliverAllVotes (Precommit, testedVal, faultyList, validatorSet, 0, 0, Nil))

// Precommit from others are delivered to all correct processes
    .then(ListDeliverAllVotes (Precommit, others, correctList, validatorSet, 0, 0, Val("blue")))
// correct ones (others and testedVal) take steps to process all incoming precommits
    .then(others.concat(faultyList).length().reps(_ => ListTakeAStep(others.concat(List(testedVal)))))

// at this point others have decided and testedVal is ready to go into next round.
    .then(valStep(testedVal))

// TODO: at this point we are stuck. Let's discuss

//    .then(ProcessDeliverAllVotes ("Precommit", testedVal, faultyList, validatorSet, 0, 0, "blue"))
 //   .then(valStep(testedVal))
 //   .then(deliverSomeProposal(testedVal))
 //   .then(valStep(testedVal))
// not sure what should happen now.
}

}
