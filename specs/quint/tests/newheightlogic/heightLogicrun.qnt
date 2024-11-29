module heightLogicrun {

import TendermintDSL.* from "../../specs/TendermintDSL"
export TendermintDSL.*

const slow : Address

val fastSet = Correct.exclude(Set(slow))
val fastList = fastSet.fold(List(), (s, x) => s.append(x))
val ValueList = Values.fold(List(), (s, x) => s.append(x))

// *************************************************************************
// state predicates
// *************************************************************************
    
def threeProcessesHaveDecided(h: Height) : bool = {
    size(system.keys().filter(validator => length(system.get(validator).es.chain) > h)) >= 3
}

def allProcessesHaveDecided(h: Height) : bool = {
    size(system.keys().filter(validator => length(system.get(validator).es.chain) > h)) == size(system.keys())
}

def listIsAtHeight(list: List[Address], h: Height) : bool = {
    length(list.select(validator => system.get(validator).es.cs.height == h)) == length(list)
}

def isAtHeight(vals: Set[Address], h: Height) : bool = {
    size(vals.filter(validator => system.get(validator).es.cs.height == h)) == size(vals)
}

// *************************************************************************
// DSL extension for heights
// *************************************************************************
    
run ListNewHeight(active, valset, h) = {
    active.length().reps(i => newHeightAction(active[i], valset, h))

}

run fastProcessesDecideAtHeight(h : Height, value: NonNilValue) : bool = {
    // all processes in fastList are synchronous in round 0 of height h
    // slow process has all messages in incomingVotes/incomingProposal but takes no step
    val prop = proposer(validatorSet, h, 0)
    setNextValueToPropose(prop, value)
    .then(reps(2, _ => ListTakeAStep(fastList)))
    .then(ListDeliverProposal(validatorList,mkProposal(prop, h, 0, value, -1)))
    .then(ListTakeAStep(fastList))
    .then(ListDeliverAllVotes (Prevote, fastList, validatorList, validatorSet, h, 0, Val(value)))
    .then(reps(length(fastList) + 2, _ => ListTakeAStep(fastList)))  // why +2 and not +1 steps??
    .then(ListDeliverAllVotes (Precommit, fastList, validatorList, validatorSet, h, 0, Val(value)))
    .then(reps(length(fastList) + 1, _ => ListTakeAStep(fastList))) 
    .expect(threeProcessesHaveDecided(h))
}

run everyoneIsFastInRound(h: Height, r: Round, value: NonNilValue) : bool = {
    val prop = proposer(validatorSet, h, r)
    setNextValueToPropose(prop, value)
    .then(reps(2, _ => ListTakeAStep(validatorList)))
    .then(ListDeliverProposal(validatorList,mkProposal(prop, h, 0, value, -1)))
    .then(ListTakeAStep(validatorList))
    .then(ListDeliverAllVotes (Prevote, validatorList, validatorList, validatorSet, h, 0, Val(value)))
    .then(reps(length(validatorList) + 2, _ => ListTakeAStep(validatorList))) // why +2 and not +1 steps?
    .then(ListDeliverAllVotes (Precommit, validatorList, validatorList, validatorSet, h, 0, Val(value)))
    .then(reps(length(validatorList) + 1, _ => ListTakeAStep(validatorList))) 
    .expect(allProcessesHaveDecided(0))
}

run fastProcessesDoNotDecide(h: Height, r: Round, value: NonNilValue) : bool = {
    val prop = proposer(validatorSet, h, r)
    setNextValueToPropose(prop, value)
    .then(reps(2, _ => ListTakeAStep(fastList)))
    .then(ListDeliverProposal(validatorList,mkProposal(prop, h, 0, value, -1)))
    .then(ListTakeAStep(fastList))
    .then(ListDeliverAllVotes (Prevote, fastList, validatorList, validatorSet, h, r, Val(value)))
    .then(reps(length(fastList) + 2, _ => ListTakeAStep(fastList)))
    // all prevotes delivered and processed at fast processes
    .expect(system.get(fastList[0]).existsTimeout())


    // WIP, continue HERE

}

// *************************************************************************
// Test runs    
// *************************************************************************

// simple normal case run
run everyoneIsFastInRound0 = {
    init
    .then(everyoneIsFastInRound(0, 0, ValueList[0]))
}

// simple normal case multi-height run
run multiHeightRun = {
    init
    .then(everyoneIsFastInRound(0, 0, ValueList[0]))
    .then(ListNewHeight(validatorList, validatorSet, 1))
    .then(everyoneIsFastInRound(1, 0, ValueList[1]))
}

// catch up at same height
run fastProcessesDecidesAtHeight0SlowOneCatchesUp = {
    // all processes are correct, v4 is slow
    // the fast process decide in round 0
    // then the slow process starts at height 0, round 0
    // it is expected that 
    init
    .then(fastProcessesDecideAtHeight(0, ValueList[0]))

    // now v4 catches up
    .then(valStep(slow))
    .then(valStep(slow))
    .expect(allProcessesHaveDecided(1))
}

// catch up at new height (2f+1 precommits)
run slowProcessAtHeight1 = {
    // all processes are fast at height 0
    // fast processes enter height 1 and decide
    // slow process catches up with decision in O(1) steps
    init
    .then(everyoneIsFastInRound(0, 0, ValueList[0]))
    .then(ListNewHeight(fastList, validatorSet, 1))
    .expect(listIsAtHeight(fastList, 1))
    .expect(listIsAtHeight([slow], 0))
    .then(fastProcessesDecideAtHeight(1, ValueList[1]))
    .expect(listIsAtHeight([slow], 0))
    // now v4 enters height 1
    .then(ListNewHeight([slow], validatorSet, 1))
    .then(valStep(slow))
    .then(valStep(slow))
    .expect(allProcessesHaveDecided(1))
}

// catch up to higher round (same height)
run catchUpToHigherRound = {
    init
    .then(fastProcessesDoNotDecide(0, 0, ValueList[0]))

}


// catch up to higher round (new height)



// no catch-up, preload votekeeper




}




