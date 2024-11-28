module heightLogicrun {

import TendermintDSL.* from "../../specs/TendermintDSL"
export TendermintDSL.*

const slow : Address

val fastSet = Correct.exclude(Set(slow))
val fastList = fastSet.fold(List(), (s, x) => s.append(x))

def threeProcessesHaveDecided() : bool = {
    size(system.keys().filter(validator => length(system.get(validator).es.chain) > 0)) >= 3
}

run fastProcessesDecidesAtHeight0SlowOneCatchesUp = {
    // all processes are correct, v4 is slow
    // the fast proceses decide in round 0
    // then the slow process starts at height 0, round 0
    // it is expected that 
    val round0proposer = proposer(validatorSet, 0, 0)
    init
    .then(setNextValueToPropose(round0proposer, "a"))
    .then(ListTakeAStep(fastList))
    .then(ListTakeAStep(fastList))
    .then(ListDeliverProposal(validatorList,mkProposal(round0proposer,0,0,"a",-1)))
    .then(ListTakeAStep(fastList))
    .then(ListDeliverAllVotes (Prevote, fastList, validatorList, validatorSet, 0, 0, Val("a")))
    .then(reps(5, _ => ListTakeAStep(fastList)))  // why 5 and not 4 steps??
    .then(ListDeliverAllVotes (Precommit, fastList, validatorList, validatorSet, 0, 0, Val("a")))
    .then(reps(4, _ => ListTakeAStep(fastList)))  // why 5 and not 4 steps??
    .expect(threeProcessesHaveDecided)

    // now v4 catches up
    .then(valStep(slow))
    .then(valStep(slow))
}


}




