// -*- mode: Bluespec; -*-

module TendermintDSL {

import statemachineAsync.* from "./statemachineAsync"
export statemachineAsync.*

val validatorList = validators.fold(List(), (s, x) => s.append(x))
val correctList = Correct.fold(List(), (s, x) => s.append(x))
val faultyList = Faulty.fold(List(), (s, x) => s.append(x))

run ListTakeAStep (active) = {
    active.length().reps(i => valStep(active[i]))
}

run ListDeliverProposal (active, proposalMsg) = {
    active.length().reps(i => deliverProposal(active[i], proposalMsg))
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


run everyoneReceivesProposal (active, valList, valset, h, r, value) = {
    val p = Proposer (valset, h, r)
    setNextValueToPropose(p, value)
    .then(ListTakeAStep(active))
    .then(all {
        assert(true),
        ListDeliverSomeProposal(active)
    })
    .then(ListTakeAStep(active))    
}

run fromPrevoteToPrecommit (prevoteSenders, prevoteReceivers, valList, valset, h, r, value) = {
    ListDeliverAllVotes("Prevote", prevoteSenders, prevoteReceivers, valset, h, r, value)
    .then(prevoteSenders.length().reps(_ => ListTakeAStep(prevoteReceivers)))
    // extra step due to timeoutprevote double step
    .then(ListTakeAStep(prevoteReceivers))
}


}