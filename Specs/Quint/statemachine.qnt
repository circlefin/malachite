// -*- mode: Bluespec; -*-

module statemachine {

import executor.* from "./executor"
import consensus.* from "./consensus"
import voteBookkeeper.* from "./voteBookkeeper"

pure def initBookKeeper (totalVotingPower: int): Bookkeeper = 
    { height: 0, totalWeight: totalVotingPower, rounds: Map() }

pure def initConsensusState (v: Address_t) : ConsensusState = {
    p: v, 
    round: -1, 
    step: "newRound", 
    height : 0,
    lockedRound: -1, 
    lockedValue: "nil", 
    validRound: -1, 
    validValue: "nil"
}

pure def initExecutor  (v: Address_t, vs: Address_t -> int) : ExecutorState = {
    // sum-up all voting powers
    val tvp = vs.keys().fold(0, (sum, key) => sum + vs.get(key))
    {
    bk : initBookKeeper(tvp),
    cs : initConsensusState(v),
    proposals: Set(),
    valset: vs,
    executedEvents: Set(),
    pendingEvents: Set(),
    started: false,
    applyvotesResult: toEvent(0, "", {name: "", value: ""}), // debug
    }
}

pure def initNode  (v: Address_t, vs: Address_t -> int) : NodeState = {
    es: initExecutor(v,vs),
    timeout: Set(),
    incomingVotes: Set(),
    incomingProposals: Set(),
}

val validators = Set("v1", "v2", "v3", "v4")
val valset = validators.mapBy(x => 1)

var system : Address_t -> NodeState
var _hist: (str, ExecutorInput, Result)
//var _histSimple: (str, str, str)

action init = all {
    system' = validators.mapBy(v => initNode(v, valset)),
    _hist' = ("INIT", defaultInput, defaultResult)
//    _histSimple' = ("INIT", "", "")
}

// Put the proposal into the inbuffers of all validators
pure def deliverProposal (sys: Address_t -> NodeState, prop: ProposeMsg_t) : Address_t -> NodeState = {
    sys.keys().mapBy(x => 
     { ...sys.get(x), incomingProposals: sys.get(x).incomingProposals.union(Set(prop)) })
}

// Put the vote into the inbuffers of all validators
pure def deliverVote (sys: Address_t -> NodeState, vote: VoteMsg_t) : Address_t -> NodeState = {
    sys.keys().mapBy(x => 
     { ...sys.get(x), incomingVotes: sys.get(x).incomingVotes.union(Set(vote)) })
}

// Record that a timeout has started at node v
pure def startTimeout (sys: Address_t -> NodeState, v: Address_t, toName: str) : Address_t -> NodeState = {
    sys.put(v, { ...sys.get(v), timeout: sys.get(v).timeout.union(Set(
        {(toName, sys.get(v).es.cs.height, sys.get(v).es.cs.round)}
    ))})
}

action valStep(v: Address_t) : bool = {
    // pick action
    val input = system.get(v).nextAction() // TODO: nextAction could go within executor boundary
    // remove action from v
    val sys1 = system.put(v, input._1)
    // call executor
    val res = executor(sys1.get(v).es, input._2)
    all {
        // update v's state after the step
        val sys = sys1.put(v, { ...sys1.get(v), es: res._1})
        // do networking
        if (res._2.name == "proposal")
            system' = deliverProposal(sys, res._2.proposal) // TODO: this is immediate
        else if (res._2.name == "votemessage") 
            system' = deliverVote(sys, res._2.voteMessage) // TODO: this is immediate
        else if (res._2.name == "timeout")
            system' = startTimeout(sys, v, res._2.timeout)
        else if (res._2.name == "skipRound")
            //skipRound should never leave the executor
            system' = sys
        else 
            system' = sys,
        _hist' = (v, input._2, res._2)
//         _histSimple' = (v, input._2.name, res._2.name)
    }
}
    
action step = {
    nondet v = oneOf(validators)
    valStep(v)
}


// action bigbang (validator: Address_t) : bool = 
//     val res = executor(system.get(validator).es, { ...defaultInput, name: "start"})
//     all {
//     // update validator's executor state
//     val sys = system.put(validator, { ...system.get(validator), es: res._1})
//     if (res._2.name == "proposal")
//         system' = deliverProposal(sys, res._2.proposal)
//     else if (res._2.name == "vote") 
//         system' = deliverVote(sys, res._2.voteMessage)
//     else if (res._2.name == "timeout")
//         system' = startTimeout(sys, validator, res._2.timeout)
//     else if (res._2.name == "skipRound")
//         //skipRound should never leave the executor
//         system' = system
//     else 
//         system' = system,
//     _hist' = (validator, { ...defaultInput, name: "start"}, res._2)
//     }

}