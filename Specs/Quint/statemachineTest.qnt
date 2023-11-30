// -*- mode: Bluespec; -*-

/*
    This contains some (non-standard) synchronous message transfer 
    semantics, that is, 
     -  upon sending, messages are put into the incoming set of the 
        validator 
     -  no faulty messages are modeled here
    The model is quite simple but might be useful to generates 
    traces.
*/

module statemachineTest {

import driver.* from "./driver"
import consensus.* from "./consensus"
import voteBookkeeper.* from "./voteBookkeeper"

val validators = Set("v1", "v2", "v3", "v4")
val validatorSet = validators.mapBy(x => 1)

var system : Address_t -> NodeState
var _hist: (str, DriverInput, ConsensusOutput)
//var _histSimple: (str, str, str)

action init = all {
    system' = validators.mapBy(v => initNode(v, validatorSet)),
    _hist' = ("INIT", defaultInput, defaultOutput)
//    _histSimple' = ("INIT", "", "")
}

// Put the proposal into the inbuffers of all validators
pure def deliverProposal (sys: Address_t -> NodeState, prop: Proposal_t) : Address_t -> NodeState = {
    sys.keys().mapBy(x => 
     { ...sys.get(x), incomingProposals: sys.get(x).incomingProposals.union(Set(prop)) })
}

// Put the vote into the inbuffers of all validators
pure def deliverVote (sys: Address_t -> NodeState, vote: Vote_t) : Address_t -> NodeState = {
    sys.keys().mapBy(x => 
     { ...sys.get(x), incomingVotes: sys.get(x).incomingVotes.union(Set(vote)) })
}

// Record that a timeout has started at node v
pure def startTimeout (sys: Address_t -> NodeState, v: Address_t, toName: str) : Address_t -> NodeState = {
    sys.put(v, { ...sys.get(v), timeout: sys.get(v).timeout.union(Set(
        {(toName, sys.get(v).es.cs.height, sys.get(v).es.cs.round)}
    ))})
}

// this is a simple semantics that puts messages that are sent immediately in the
// inbuffer of the receivers. By the way nextAction() is implemented timeouts
// hardly ever are fired
action valStep(v: Address_t) : bool = {
    // pick action
    val input = system.get(v).nextAction() // TODO: nextAction could go within driver boundary
    // remove action from v
    val sys1 = system.put(v, input._1)
    // call driver
    val res = driver(sys1.get(v).es, input._2)
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
            //skipRound should never leave the driver
            system' = sys
        else 
            system' = sys,
        _hist' = (v, input._2, res._2)
//         _histSimple' = (v, input._2.name, res._2.name)
    }
}

action setNextValueToPropose(v: Address_t, value: Value_t) : bool = all {
    val res = driver(system.get(v).es, { ...defaultInput, name: "SetNextProposedValue", nextValueToPropose: value})
    val newNS = { ...system.get(v), es: res._1}
    system' = system.put(v, newNS),
     _hist' = _hist
}

action step = {
    nondet v = oneOf(validators)
    nondet value = oneOf(Set("a", "b", "c"))
    any {
        valStep(v),
        setNextValueToPropose(v, value),
    }
}



action valStepCommand(v: Address_t, command: str) : bool = {
    // pick action
    val input = system.get(v).nextActionCommand(command) 
    // remove action from v
    val sys1 = system.put(v, input._1)
    // call driver
    val res = driver(sys1.get(v).es, input._2)
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
            //skipRound should never leave the driver
            system' = sys
        else 
            system' = sys,
        _hist' = (v, input._2, res._2)
//         _histSimple' = (v, input._2.name, res._2.name)
    }
}

run DecidingRunTest = {
    init
    .then(setNextValueToPropose("v2", "a block"))
    .then(valStepCommand("v1", "start"))
    .then(valStepCommand("v2", "start"))
    .then(valStepCommand("v3", "start"))
    .then(valStepCommand("v4", "start"))
    .then(valStepCommand("v1", "proposal"))
    .then(valStepCommand("v2", "proposal"))
    .then(valStepCommand("v3", "proposal"))
    .then(valStepCommand("v4", "proposal"))  
    .then(valStepCommand("v1", "vote"))
    .then(valStepCommand("v1", "vote"))
    .then(valStepCommand("v1", "vote"))
    .then(all{
        assert(system.get("v1").incomingVotes.contains(
             { src: "v1", height: 0, round: 0, step: "Precommit", id: "a block" }
        )),
        valStepCommand("v2", "vote")
        })
    .then(valStepCommand("v2", "vote"))
    .then(valStepCommand("v2", "vote"))
    .then(valStepCommand("v2", "vote"))
    .then(all{
        assert(system.get("v2").incomingVotes.contains(
             { src: "v2", height: 0, round: 0, step: "Precommit", id: "a block" }
        )),
        valStepCommand("v3", "vote")
        })
    .then(valStepCommand("v3", "vote"))
    .then(valStepCommand("v3", "vote"))
    .then(valStepCommand("v3", "vote"))
    .then(valStepCommand("v3", "vote"))
    .then(all{
        assert(system.get("v3").incomingVotes.contains(
             { src: "v3", height: 0, round: 0, step: "Precommit", id: "a block" }
        )),
        assert(system.get("v3").es.chain == List()),
        valStepCommand("v3", "vote")
        })
    .then(all{
    // validator 3 decided on "a block"
    assert(system.get("v3").es.chain.head() == "a block"),
    system' = system,
    _hist' = _hist
    })    
}


run TimeoutRunTest = {
    init
    .then(setNextValueToPropose("v2", "a block"))
    .then(valStepCommand("v1", "start"))
    .then(valStepCommand("v2", "start"))
    .then(valStepCommand("v3", "start"))
    .then(valStepCommand("v4", "start"))
    .then(valStepCommand("v1", "timeout"))
    .then(valStepCommand("v2", "proposal"))
    .then(valStepCommand("v3", "timeout"))
    .then(valStepCommand("v4", "timeout"))
    .then(valStepCommand("v4", "vote"))
    .then(valStepCommand("v4", "vote"))
    .then(valStepCommand("v4", "vote"))
    .then(valStepCommand("v4", "vote"))    
    .then(valStepCommand("v3", "vote"))
    .then(valStepCommand("v3", "vote"))
    .then(valStepCommand("v3", "vote"))
    .then(valStepCommand("v3", "vote"))
    .then(valStepCommand("v3", "vote"))     
    .then(valStepCommand("v2", "vote"))    
    .then(valStepCommand("v2", "vote"))
    .then(valStepCommand("v2", "vote"))
    .then(valStepCommand("v2", "vote"))
    .then(valStepCommand("v2", "vote"))
    .then(valStepCommand("v2", "vote"))    
    .then(valStepCommand("v1", "vote"))     
    .then(valStepCommand("v1", "vote"))    
    .then(valStepCommand("v1", "vote"))
    .then(valStepCommand("v1", "vote"))
    .then(valStepCommand("v1", "vote"))
    .then(valStepCommand("v1", "vote"))
    .then(valStepCommand("v1", "vote"))
    .then(valStepCommand("v4", "vote"))
    .then(valStepCommand("v4", "vote"))
    .then(valStepCommand("v4", "vote"))
    .then(valStepCommand("v4", "vote"))
    .then(valStepCommand("v4", "vote"))
    .then(valStepCommand("v4", "timeout"))
    .then(all {
        // validator 4 timed out and went to round 1
        assert(system.get("v4").es.cs.round == 1),
        system' = system,
        _hist' = _hist,
    }) 
}

}

