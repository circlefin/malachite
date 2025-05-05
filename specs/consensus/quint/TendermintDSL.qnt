// -*- mode: Bluespec; -*-

module TendermintDSL {

import types.* from "./types"
export types.*
import statemachineAsync.* from "./statemachineAsync"
export statemachineAsync.*

val validatorList = validators.fold(List(), (s, x) => s.append(x))
val correctList = Correct.fold(List(), (s, x) => s.append(x))
val faultyList = Faulty.fold(List(), (s, x) => s.append(x))

def SetFromList(L) = {
    L.foldl(Set(), (s, x) => s.union(Set(x)))
}

run ListTakeAStep (active) = {
    active.length().reps(i => valStep(active[i]))
}

run ListTimesOut (active) = {
    active.length().reps(i => valTimeout(active[i]))
}

run ListDeliverProposal (active, propMsg) = {
    all{
        assert( Faulty.contains(propMsg.srcAddress) or 
                SetFromList(active).forall(proc => propBuffer.get(proc).contains(propMsg))),
        active.length().reps(i => deliverProposal(active[i], propMsg))
    }
}

run ListDeliverVote (active, voteMsg) = {
    active.length().reps(i => deliverVote(active[i], voteMsg))
}

run ListDeliverSomeProposal (active) = {
    all{
        assert( Faulty != Set() or 
                SetFromList(active).forall(proc => propBuffer.get(proc) != Set())),
        active.length().reps(i => deliverSomeProposal(active[i]))
    }
}

run ProcessDeliverAllVotes (voteType, recipient, fromList, valset, h, r, value) = {
    all {
        assert( SetFromList(fromList).forall(proc => 
                    Faulty.contains(proc) or
                    voteBuffer.get(recipient).contains(mkVote(voteType, proc, h, r, value))
        )),
        fromList.length().reps(i => deliverVote(recipient, mkVote(voteType, fromList[i], h, r, value)))
    }
}

run ListDeliverAllVotes(voteType, fromList, toList, valset, h, r, value) =
    all {
        assert( SetFromList(fromList).forall(proc => 
                    Faulty.contains(proc) or
                    SetFromList(toList).forall(recp => voteBuffer.get(recp).contains(mkVote(voteType, proc, h, r, value)))
        )),
        toList.length().reps(i => ProcessDeliverAllVotes(voteType, toList[i], fromList, valset, h, r, value))
    }
    
run onlyProposerReceivesProposal(active, valList, valset, h, r, value) = {
    val p = proposer(valset, h, r)
    setNextValueToPropose(p, value)
    .then(ListTakeAStep(active))
    .then(all{ // after new round an empty step to clean step "propose"
        assert(SetFromList(active).forall(proc => system.get(proc).es.pendingStepChange == ProposeStep)),
        ListTakeAStep(active)
    })
    .then(deliverSomeProposal(p))
    .then(valStep(p))
    .then(ListTimesOut(active.select(x => x != p)))
    .then(all{ 
        assert(SetFromList(active).forall(proc => system.get(proc).es.pendingStepChange == PrevoteStep)),
        ListTakeAStep(active)
    })            
}

// TODO: add valid round more cleanly on one run for all cases
run everyoneReceivesProposalVR (active, valList, valset, h, r, value, vr) = {
    val p = proposer(valset, h, r)
    val propMsg = mkProposal(p, h, r, value, vr)
    all{
        setNextValueToPropose(p, value),
        assert(ListContains(active, p)) // proposer must be in active list for this run
    }
    .then(ListTimesOut(active))
    .then(all{ // after new round an empty step to clean step "propose"
        assert(SetFromList(active).forall(proc => system.get(proc).es.pendingStepChange == ProposeStep)),
        ListTakeAStep(active)
    })
    .then(all {
        assert(true),
        ListDeliverProposal(active, propMsg)
    })
    .then(ListTakeAStep(active))
    .then(all{ 
        assert(SetFromList(active).forall(proc => system.get(proc).es.pendingStepChange == PrevoteStep)),
        ListTakeAStep(active)
    })        
}

run everyoneReceivesProposal (active, valList, valset, h, r, value) = {
    val p = proposer(valset, h, r)
    val propMsg = mkProposal(p, h, r, value, -1)
    all{
        setNextValueToPropose(p, value),
        assert(ListContains(active, p)) // proposer must be in active list for this run
    }
    .then(ListTakeAStep(active))
    .then(all{ // after new round an empty step to clean step "propose"
        assert(SetFromList(active).forall(proc => system.get(proc).es.pendingStepChange == ProposeStep)),
        ListTakeAStep(active)
    })
    .then(all {
        assert(true),
        ListDeliverProposal(active, propMsg)
    })
    .then(ListTakeAStep(active))
    .then(all{ 
        assert(SetFromList(active).forall(proc => system.get(proc).es.pendingStepChange == PrevoteStep)),
        ListTakeAStep(active)
    })        
}

run everyoneReceivesProposalTO (active, valList, valset, h, r, value) = {
    val p = proposer(valset, h, r)
    val propMsg = mkProposal(p, h, r, value, -1)
    all{
        setNextValueToPropose(p, value),
        assert(ListContains(active, p)) // proposer must be in active list for this run
    }
    .then(ListTimesOut(active))
    .then(all{ // after new round an empty step to clean step "propose"
        assert(SetFromList(active).forall(proc => system.get(proc).es.pendingStepChange == ProposeStep)),
        ListTakeAStep(active)
    })
    .then(all {
        assert(true),
        ListDeliverProposal(active, propMsg)
    })
    .then(ListTakeAStep(active))
    .then(all{ 
        assert(SetFromList(active).forall(proc => system.get(proc).es.pendingStepChange == PrevoteStep)),
        ListTakeAStep(active)
    })        
}

run fromPrevoteToPrecommit(prevoteSenders, prevoteReceivers, valList, valset, h, r, value) = {
    ListDeliverAllVotes(Prevote, prevoteSenders, prevoteReceivers, valset, h, r, value)
    .then(prevoteSenders.length().reps(_ => ListTakeAStep(prevoteReceivers)))
    // extra step due to timeoutprevote double step
    .then(ListTakeAStep(prevoteReceivers))
    .then(all{ 
        assert(SetFromList(prevoteReceivers).forall(proc => system.get(proc).es.pendingStepChange == PrecommitStep)),
        ListTakeAStep(prevoteReceivers)
    })
}

}
