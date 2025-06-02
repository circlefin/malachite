module heightLogicrun {

import TendermintDSL.* from "../../TendermintDSL"
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

run fastProcessesDoNotDecide(h: Height, r: Round, value: NonNilValue, value2: NonNilValue) : bool = {
    //val nextProposer = proposer(validatorSet, h, 1)
    val thisProposer = proposer(validatorSet, h, r)
    init
    .then(onlyProposerReceivesProposal(fastList, validatorList, validatorSet, h, r, value))
    .then(ListDeliverAllVotes(Prevote, List(thisProposer), fastList, validatorSet, h, r, Val(value)))
    .then(ListDeliverAllVotes(Prevote, fastList.select(x => x != thisProposer), fastList, validatorSet, h, r, Nil)) 
    .then(fastList.length().reps(_ => ListTakeAStep(fastList)))   
    .then(all{
         assert(SetFromList(fastList).forall(proc => system.get(proc).timeouts.contains((PrevoteTimeout, h, r)))),
         ListTimesOut(fastList)
    })
    .then(all{ 
        assert(SetFromList(fastList).forall(proc => system.get(proc).es.pendingStepChange == PrecommitStep)),
        ListTakeAStep(fastList)
    })
    .then(ListDeliverAllVotes(Precommit, fastList, fastList, validatorSet, h, r, Nil))
    .then(fastList.length().reps(_ => ListTakeAStep(fastList)))   
    // FastList now go to next round on timeoutPrecommit
    .then(all{
        assert(SetFromList(fastList).forall(proc => system.get(proc).timeouts.contains((PrecommitTimeout, h, r)))),
        everyoneReceivesProposalTO (fastList, validatorList, validatorSet, h, r + 1, value2)
    })        
    .then(fromPrevoteToPrecommit(fastList, fastList, validatorList, validatorSet, r, r + 1, Val(value2)))
    .then(ListDeliverAllVotes(Precommit, fastList, fastList, validatorSet, h, r+ 1, Val(value2)))
    .then(fastList.length().reps(_ => ListTakeAStep(fastList)))
    .then(all {
        assert(SetFromList(fastList).forall(proc => system.get(proc).es.chain[h].decision.proposal == value2)),
        unchangedAll
    })       
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
    .expect(allProcessesHaveDecided(0))
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
    .then(fastProcessesDoNotDecide(0, 0, ValueList[0], ValueList[1]))
    // deliver all prevotes of round 0 and 1
    .then(ListDeliverAllVotes(Prevote, ["v1", "v3"], [slow], validatorSet, 0, 0, Nil))
    .then(ListDeliverAllVotes(Prevote, ["v2"], [slow], validatorSet, 0, 0, Val("a")))
    .then(ListDeliverAllVotes(Prevote, fastList, [slow], validatorSet, 0, 1, Val("b")))
    .then(valStep(slow))
    .then(valStep(slow))
    .then(valStep(slow))
}

// catch up to higher round (new height)
run catchUpToHigherRoundAfterHeightChange = {
    val h1r0proposer = proposer(validatorSet, 1, 0)
    val value = "a"
    val value2 = "b"

    init
    // height 0
    .then(everyoneIsFastInRound(0, 0, ValueList[0]))
    .expect(allProcessesHaveDecided(0))
    .then(ListNewHeight(fastList, validatorSet, 1))

    // height 1 with unsuccessful round 0
    .then(onlyProposerReceivesProposal(fastList, validatorList, validatorSet, 1, 0, value))
    .then(ListDeliverAllVotes(Prevote, List(h1r0proposer), fastList, validatorSet, 1, 0, Val(value)))
    .then(ListDeliverAllVotes(Prevote, fastList.select(x => x != h1r0proposer), fastList, validatorSet, 1, 0, Nil)) 
    .then(fastList.length().reps(_ => ListTakeAStep(fastList)))   
    .then(all{
         assert(SetFromList(fastList).forall(proc => system.get(proc).timeouts.contains((PrevoteTimeout, 1, 0)))),
         ListTimesOut(fastList)
    })
    .then(all{ 
        assert(SetFromList(fastList).forall(proc => system.get(proc).es.pendingStepChange == PrecommitStep)),
        ListTakeAStep(fastList)
    })
    .then(ListDeliverAllVotes(Precommit, fastList, fastList, validatorSet, 1, 0, Nil))
    .then(fastList.length().reps(_ => ListTakeAStep(fastList)))   
    // FastList now go to next round on timeoutPrecommit
    .then(all{
        assert(SetFromList(fastList).forall(proc => system.get(proc).timeouts.contains((PrecommitTimeout, 1, 0)))),
        everyoneReceivesProposalTO (fastList, validatorList, validatorSet, 1, 1, value2)
    })        
    .then(fromPrevoteToPrecommit(fastList, fastList, validatorList, validatorSet, 1, 1, Val(value2)))
    .then(ListDeliverAllVotes(Precommit, fastList, fastList, validatorSet, 1, 1, Val(value2)))
    .then(ListNewHeight([slow], validatorSet, 1))
    .then(ListDeliverAllVotes(Precommit, fastList, [slow], validatorSet, 1, 0, Nil))
    .then(ListDeliverAllVotes(Prevote, fastList, [slow], validatorSet, 1, 1, Val(value2)))
    .then(ListDeliverAllVotes(Precommit, fastList, [slow], validatorSet, 1, 1, Val(value2)))
    .then(valStep(slow))
    .expect(system.get(slow).es.cs.round == 1)
}

run preloadVotekeeperOnly = {
    val prop = proposer(validatorSet, 0, 0)
    val value = "a"
    init
    .then(setNextValueToPropose(prop, value))
    .then(reps(2, _ => ListTakeAStep(fastList)))
    .then(ListDeliverProposal(validatorList,mkProposal(prop, 0, 0, value, -1)))
    .then(ListTakeAStep(fastList))
    .then(ListDeliverAllVotes (Prevote, fastList, validatorList, validatorSet, 0, 0, Val(value)))
    // now slow catches up
    // new height logic (StartDInput)
    .then(valStep(slow)) 
    // StepChangeDInput
    .then(valStep(slow))
    // ProposalDInput
    .then(valStep(slow))
    .then(valStep(slow))
    .expect(system.get(slow).es.cs.step == PrecommitStep)
}
}

