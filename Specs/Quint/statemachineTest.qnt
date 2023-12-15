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

import types.* from "./types"
import extraSpells.* from "./extraSpells"
import driver.* from "./driver"
import consensus.* from "./consensus"
import voteBookkeeper.* from "./voteBookkeeper"

val validators = Set("v1", "v2", "v3", "v4")
val validatorSet = validators.mapBy(x => 1)

var system : Address -> NodeState
var _hist: (str, DriverInput, ConsensusOutput)
//var _histSimple: (str, str, str)

action init = all {
    system' = validators.mapBy(v => initNode(v, validatorSet)),
    _hist' = ("INIT", NoDInput, NoConsensusOutput)
//    _histSimple' = ("INIT", "", "")
}

// Put the proposal into the inbuffers of all validators
pure def deliverProposal(sys: Address -> NodeState, prop: Proposal): Address -> NodeState =
    sys.keys().mapBy(x => 
        { ...sys.get(x), incomingProposals: sys.get(x).incomingProposals.union(Set(prop)) })

// Put the vote into the inbuffers of all validators
pure def deliverVote(sys: Address -> NodeState, vote: Vote): Address -> NodeState =
    sys.keys().mapBy(x => 
        { ...sys.get(x), incomingVotes: sys.get(x).incomingVotes.union(Set(vote)) })

// Record that a timeout has started at node v
pure def startTimeout(sys: Address -> NodeState, v: Address, timeout: Timeout): Address -> NodeState =
    val newTimeout = (timeout, sys.get(v).es.cs.height, sys.get(v).es.cs.round)
    sys.put(v, { ...sys.get(v), timeouts: sys.get(v).timeouts.union(Set(newTimeout))})

// this is a simple semantics that puts messages that are sent immediately in the
// inbuffer of the receivers. By the way nextAction() is implemented timeouts
// hardly ever are fired
action valStep(v: Address): bool =
    // pick action
    val next_action = system.get(v).nextAction() // TODO: nextAction could go within driver boundary
    val node_state = next_action._1
    val driver_input = next_action._2
    // remove action from v
    val sys1 = system.put(v, node_state)
    // call driver
    val res = driver(sys1.get(v).es, driver_input)
    val driver_state = res._1
    val consensus_output = res._2
    all {
        // update v's state after the step
        val sys = sys1.put(v, { ...sys1.get(v), es: driver_state})
        // do networking
        match consensus_output {
            | ProposalOutput(proposal) =>
                system' = deliverProposal(sys, proposal) // TODO: this is immediate
            | VoteOutput(vote) =>
                system' = deliverVote(sys, vote) // TODO: this is immediate
            | TimeoutOutput(timeout) =>
                system' = startTimeout(sys, v, timeout._2)
            | SkipRoundOutput(round) => // CHECK: round is not used
                //skipRound should never leave the driver
                system' = sys
            | _ =>
                system' = sys
        },
        _hist' = (v, driver_input, consensus_output)
//         _histSimple' = (v, input._2.name, res._2.name)
    }

action setNextValueToPropose(v: Address, value: NonNilValue): bool = all {
    val res = driver(system.get(v).es, SetNextProposedValueDInput(value))
    val newNS = { ...system.get(v), es: res._1}
    system' = system.put(v, newNS),
    _hist' = _hist
}

action step =
    nondet v = oneOf(validators)
    nondet value = oneOf(Set("a", "b", "c"))
    any {
        valStep(v),
        setNextValueToPropose(v, value),
    }

action valStepCommand(v: Address, command: Command): bool =
    // pick action
    val input = system.get(v).nextActionCommand(command) 
    // remove action from v
    val sys1 = system.put(v, input._1)
    // call driver
    val res = driver(sys1.get(v).es, input._2)
    val driver_state = res._1
    val consensus_output = res._2
    all {
        // update v's state after the step
        val sys = sys1.put(v, { ...sys1.get(v), es: driver_state})
        // do networking
        match consensus_output {
            | ProposalOutput(proposal) =>
                system' = deliverProposal(sys, proposal) // TODO: this is immediate
            | VoteOutput(vote) =>
                system' = deliverVote(sys, vote) // TODO: this is immediate
            | TimeoutOutput(timeout) =>
                system' = startTimeout(sys, v, timeout._2)
            | SkipRoundOutput(round) => // CHECK: round is not used
                //skipRound should never leave the driver
                system' = sys
            | _ =>
                system' = sys
        },
        _hist' = (v, input._2, consensus_output)
//         _histSimple' = (v, input._2.name, res._2.name)
    }

run DecidingRunTest =
    init
    .then(setNextValueToPropose("v2", "a block"))
    .then(valStepCommand("v1", StartCmd))
    .then(valStepCommand("v2", StartCmd))
    .then(valStepCommand("v3", StartCmd))
    .then(valStepCommand("v4", StartCmd))
    .then(valStepCommand("v1", ProposalCmd))
    .then(valStepCommand("v2", ProposalCmd))
    .then(valStepCommand("v3", ProposalCmd))
    .then(valStepCommand("v4", ProposalCmd))  
    .then(valStepCommand("v1", VoteCmd))
    .then(valStepCommand("v1", VoteCmd))
    .then(valStepCommand("v1", VoteCmd))
    .then(valStepCommand("v1", PendingCmd))
    .then(all{
        assert(system.get("v1").incomingVotes.contains(mkVote(Precommit, "v1", 0, 0, Val("a block")))),
        valStepCommand("v2", VoteCmd)
        })
    .then(valStepCommand("v2", VoteCmd))
    .then(valStepCommand("v2", VoteCmd))
    .then(valStepCommand("v2", VoteCmd))
    .then(valStepCommand("v2", PendingCmd))
    .then(all{
        assert(system.get("v2").incomingVotes.contains(mkVote(Precommit, "v2", 0, 0, Val("a block")))),
        valStepCommand("v3", VoteCmd)
        })
    .then(valStepCommand("v3", VoteCmd))
    .then(valStepCommand("v3", VoteCmd))
    .then(valStepCommand("v3", VoteCmd))
    .then(valStepCommand("v3", VoteCmd))
    .then(valStepCommand("v3", PendingCmd))
    .then(all{
        assert(system.get("v3").incomingVotes.contains(mkVote(Precommit, "v3", 0, 0, Val("a block")))),
        assert(system.get("v3").es.chain == List()),
        valStepCommand("v3", VoteCmd)
        })
    .then(all{
        // validator 3 decided on "a block"
        assert(system.get("v3").es.chain.head() == Val("a block")),
        system' = system,
        _hist' = _hist
    })

run TimeoutRunTest = {
    init
    .then(setNextValueToPropose("v2", "a block"))
    .then(valStepCommand("v1", StartCmd))
    .then(valStepCommand("v2", StartCmd))
    .then(valStepCommand("v3", StartCmd))
    .then(valStepCommand("v4", StartCmd))
    .then(valStepCommand("v1", TimeoutCmd))
    .then(valStepCommand("v2", ProposalCmd))
    .then(valStepCommand("v3", TimeoutCmd))
    .then(valStepCommand("v4", TimeoutCmd))
    .then(valStepCommand("v4", VoteCmd))
    .then(valStepCommand("v4", VoteCmd))
    .then(valStepCommand("v4", VoteCmd))
    .then(valStepCommand("v4", VoteCmd))    
    .then(valStepCommand("v3", VoteCmd))
    .then(valStepCommand("v3", VoteCmd))
    .then(valStepCommand("v3", VoteCmd))
    .then(valStepCommand("v3", VoteCmd))
    .then(valStepCommand("v3", VoteCmd))     
    .then(valStepCommand("v2", VoteCmd))    
    .then(valStepCommand("v2", VoteCmd))
    .then(valStepCommand("v2", VoteCmd))
    .then(valStepCommand("v2", VoteCmd))
    .then(valStepCommand("v2", VoteCmd))
    .then(valStepCommand("v2", VoteCmd))    
    .then(valStepCommand("v1", VoteCmd))     
    .then(valStepCommand("v1", VoteCmd))    
    .then(valStepCommand("v1", VoteCmd))
    .then(valStepCommand("v1", VoteCmd))
    .then(valStepCommand("v1", VoteCmd))
    .then(valStepCommand("v1", VoteCmd))
    .then(valStepCommand("v1", VoteCmd))
    .then(valStepCommand("v4", VoteCmd))
    .then(valStepCommand("v4", VoteCmd))
    .then(valStepCommand("v4", VoteCmd))
    .then(valStepCommand("v4", VoteCmd))
    .then(valStepCommand("v4", VoteCmd))
    .then(valStepCommand("v4", TimeoutCmd))
    .then(all {
        // validator 4 timed out and went to round 1
        assert(system.get("v4").es.cs.round == 1),
        system' = system,
        _hist' = _hist,
    }) 
}

}

