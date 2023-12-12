// -*- mode: Bluespec; -*-

module TendermintDSL {

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

run ListDeliverProposal (active, propMsg) = {
    active.length().reps(i => deliverProposal(active[i], propMsg))
}

run ListDeliverSomeProposal (active) = {
    active.length().reps(i => deliverSomeProposal(active[i]))
}

run ProcessDeliverAllVotes  (cstep, recepient, fromList, valset, h, r, value) = {
    fromList.length().reps(i => deliverVote(recepient, { src: fromList[i], height: h, round: r, step: cstep, id: value }))
}

run ListDeliverAllVotes (cstep, fromList, toList, valset, h, r, value) = {
    toList.length().reps(i => ProcessDeliverAllVotes (cstep, toList[i], fromList, valset, h, r, value))
}

run onlyProposerReceivesProposal (active, valList, valset, h, r, value) = {
    val p = Proposer (valset, h, r)
    setNextValueToPropose(p, value)
    .then(ListTakeAStep(active))
    .then(all{ // after new round an empty step to clean step "propose"
        assert(SetFromList(active).forall(proc => system.get(proc).es.pendingStepChange == "propose")),
        ListTakeAStep(active)
    })
    .then(deliverSomeProposal(p))
    .then(ListTakeAStep(active))
    .then(all{ 
        assert(SetFromList(active).forall(proc => system.get(proc).es.pendingStepChange == "Prevote")),
        ListTakeAStep(active)
    })            
}

// TODO: add valid round more cleanly on one run for all cases
run everyoneReceivesProposalVR (active, valList, valset, h, r, value, vr) = {
    val p = Proposer (valset, h, r)
    val propMsg = { height: h, proposal: value, round: r, src: p, validRound: vr }
    setNextValueToPropose(p, value)
    .then(ListTakeAStep(active))
    .then(all{ // after new round an empty step to clean step "propose"
        assert(SetFromList(active).forall(proc => system.get(proc).es.pendingStepChange == "propose")),
        ListTakeAStep(active)
    })
    .then(all {
        assert(true),
        ListDeliverProposal(active, propMsg)
    })
    .then(ListTakeAStep(active))
    .then(all{ 
        assert(SetFromList(active).forall(proc => system.get(proc).es.pendingStepChange == "Prevote")),
        ListTakeAStep(active)
    })        
}

run everyoneReceivesProposal (active, valList, valset, h, r, value) = {
    val p = Proposer (valset, h, r)
    val propMsg = { height: h, proposal: value, round: r, src: p, validRound: -1 }
    setNextValueToPropose(p, value)
    .then(ListTakeAStep(active))
    .then(all{ // after new round an empty step to clean step "propose"
        assert(SetFromList(active).forall(proc => system.get(proc).es.pendingStepChange == "propose")),
        ListTakeAStep(active)
    })
    .then(all {
        assert(true),
        ListDeliverProposal(active, propMsg)
    })
    .then(ListTakeAStep(active))
    .then(all{ 
        assert(SetFromList(active).forall(proc => system.get(proc).es.pendingStepChange == "Prevote")),
        ListTakeAStep(active)
    })        
}

run fromPrevoteToPrecommit (prevoteSenders, prevoteReceivers, valList, valset, h, r, value) = {
    ListDeliverAllVotes("Prevote", prevoteSenders, prevoteReceivers, valset, h, r, value)
    .then(prevoteSenders.length().reps(_ => ListTakeAStep(prevoteReceivers)))
    // extra step due to timeoutprevote double step
    .then(ListTakeAStep(prevoteReceivers))
   .then(all{ 
       assert(SetFromList(prevoteReceivers).forall(proc => system.get(proc).es.pendingStepChange == "Precommit")),
       ListTakeAStep(prevoteReceivers)
   })            
}


}