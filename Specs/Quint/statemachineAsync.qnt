// -*- mode: Bluespec; -*-

/*
    This contains asynchronous message transfer semantics, that is, 
     -  upon sending, messages are put into a buffer (for each receiver). 
        The buffer is part of the network and not in any validator state
     -  there is a deliver event that takes a message out of the buffer
        and puts it into the incoming set of the validator (alternatively
        a message by a faulty process may be put into the incoming set)
     -  this allows re-ordering of message in the network, that is, a 
        process may receive message m1 before m2 while another process 
        may receive m2 before m1
    Example models using this specification can be found in AsyncModels.qnt
*/


module statemachineAsync {

import types.* from "./types"
import driver.* from "./driver"
export driver.* 
import consensus.* from "./consensus"
export consensus.* 
import voteBookkeeper.* from "./voteBookkeeper"

const validators: Set[Address]
const validatorSet: Address -> Weight
const Faulty: Set[Address]
val Correct = validators.exclude(Faulty)

// These are used to define what messages can be sent by faulty validators
const Values: Set[NonNilValue]
const Rounds: Set[Round]
const Heights: Set[Height]

// putting all messages that could be ever sent by faulty validators into 
// AllFaultyVotes and AllFaultyProposals

val RoundsOrNil = Rounds.union(Set(-1))
val VoteTypes = Set(Prevote, Precommit)

val AllFaultyVotes: Set[Vote] = 
    tuples(VoteTypes, Faulty, Heights, Rounds, Values.map(v => Val(v)).union(Set(Nil)))
        .map(t => mkVote(t._1, t._2, t._3, t._4, t._5))

val AllFaultyProposals : Set[Proposal] =
    tuples(Faulty, Heights, Rounds, Values, RoundsOrNil)
        .map(t => mkProposal(t._1, t._2, t._3, t._4, t._5))

// Global State
var system : Address -> NodeState
var propBuffer : Address -> Set[Proposal]
var voteBuffer : Address -> Set[Vote]
var _hist: { validator: Address, input: DriverInput, output: ConsensusOutput }

action unchangedAll = all {
    system' = system,
    propBuffer' = propBuffer,
    voteBuffer' = voteBuffer,
    _hist' = _hist
}

val AgreementInv = tuples(Correct, Correct).forall(p => 
    (system.get(p._1).es.chain != List() and system.get(p._2).es.chain != List())
        implies
            system.get(p._1).es.chain[0] == system.get(p._2).es.chain[0])

// Actions
action init = all {
    system' = Correct.mapBy(v => initNode(v, validatorSet)),
    propBuffer' = Correct.mapBy(v => Set()),
    voteBuffer' = Correct.mapBy(v => Set()),
    _hist' = { validator: "INIT", input: NoDInput, output: NoConsensusOutput }
}

// Put the proposal into the buffers of all validators
pure def sendProposal(buffer: Address -> Set[Proposal], prop: Proposal): Address -> Set[Proposal] =
    buffer.keys().mapBy(x => { buffer.get(x).union(Set(prop)) })

// Put the vote into the inbuffers of all validators
pure def sendVote(sys: Address -> Set[Vote], vote: Vote): Address -> Set[Vote] =
    sys.keys().mapBy(x => { ...sys.get(x).union(Set(vote)) })

// Record that a timeout has started at node v
// As we do set union here, there is no need to check whether this happens the first time
pure def startTimeout(sys: Address -> NodeState, v: Address, timeout: Timeout) : Address -> NodeState =
    val newTimeout = (timeout, sys.get(v).es.cs.height, sys.get(v).es.cs.round)
    sys.put(v, { ...sys.get(v), timeouts: sys.get(v).timeouts.union(Set(newTimeout)) })

action valStep(v: Address) : bool = {
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
            | ProposalOutput(proposal) => all {
                // TODO: do we need to start a timeout on this result, that is, when we are proposer and
                // send a proposal?
                propBuffer' = sendProposal(propBuffer, proposal), 
                voteBuffer' = voteBuffer,
                system' = sys,
            }
            | VoteOutput(vote) => all {
                propBuffer' = propBuffer,
                voteBuffer' = sendVote(voteBuffer, vote), 
                system' = sys,
            }
            | TimeoutOutput(timeout) => all {
                propBuffer' = propBuffer,
                voteBuffer' = voteBuffer,
                system' = startTimeout(sys, v, timeout),
            }
            | SkipRoundOutput(round) => all { // CHECK: round is not used
                propBuffer' = propBuffer,
                voteBuffer' = voteBuffer,
                //skipRound should never leave the driver
                system' = sys,
            }
            | _ => all {
                propBuffer' = propBuffer,
                voteBuffer' = voteBuffer,
                system' = sys,
            }
        },
        _hist' = { validator: v, input: driver_input, output: consensus_output }
//         _histSimple' = (v, input._2.name, res._2.name)
    }
}

action setNextValueToPropose(v: Address, value: NonNilValue) : bool = all {
    val res = driver(system.get(v).es, SetNextProposedValueDInput(value))
    val newNS = { ...system.get(v), es: res._1}
    system' = system.put(v, newNS),
    _hist' = _hist,
    propBuffer' = propBuffer,
    voteBuffer' = voteBuffer,
}

action deliverProposal(v: Address, p: Proposal) : bool = all {
    propBuffer.get(v).union(AllFaultyProposals).contains(p), // the proposal must be sent or from a faulty node
    propBuffer' = propBuffer.put(v, propBuffer.get(v).exclude(Set(p))),
    system' = system.put(v, { ...system.get(v), incomingProposals: system.get(v).incomingProposals.union(Set(p)) }),
    _hist' = _hist,
    voteBuffer' = voteBuffer, 
}

action deliverSomeProposal(v: Address) : bool = any {
    nondet prop = oneOf(propBuffer.get(v))
    deliverProposal(v, prop)
}

action deliverVote(v: Address, vote: Vote) : bool = all {
    voteBuffer.get(v).union(AllFaultyVotes).contains(vote), // the vote must be sent or from a faulty node
    voteBuffer' = voteBuffer.put(v, voteBuffer.get(v).exclude(Set(vote))),
    system' = system.put(v, { ...system.get(v), incomingVotes: system.get(v).incomingVotes.union(Set(vote)) }),
    _hist' = _hist,
    propBuffer' = propBuffer, 
}

// deliver a message. Take it from the network buffer of from the faulty set
//  and put it into the incoming sets
action deliver(v: Address) : bool = any {
    nondet prop = oneOf(propBuffer.get(v).union(AllFaultyProposals))
    deliverProposal(v, prop),
    nondet vote = oneOf(voteBuffer.get(v).union(AllFaultyVotes))
    deliverVote(v, vote)
}

action step = {
    nondet v = oneOf(Correct)
    nondet value = oneOf(Values)
    any {
        valStep(v),
        deliver(v),
        setNextValueToPropose(v, value),
    }
}


}
