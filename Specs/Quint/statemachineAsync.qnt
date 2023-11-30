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

import driver.* from "./driver"
export driver.* 
import consensus.* from "./consensus"
export consensus.* 
import voteBookkeeper.* from "./voteBookkeeper"

const validators : Set[Address_t]
const validatorSet : Address_t -> int
const Faulty  : Set[Address_t]
val Correct = validators.exclude(Faulty)

// These are used to define what messages can be sent by faulty validators
const Values : Set[Value_t]
const Rounds : Set[Round_t]
const Heights : Set[Height_t]

// putting all messages that could be ever sent by faulty validators into 
// AllFaultyVotes and AllFaultyProposals

val RoundsOrNil = Rounds.union(Set(-1))
val Steps = Set("Prevote", "Precommit")

val AllFaultyVotes : Set[Vote_t] = 
    tuples(Faulty, Heights, Rounds, Values, Steps)
    .map(t => { src: t._1, height: t._2, round: t._3, id: t._4, step: t._5 })

// val AllFaultyVotes : Set[Vote_t] = 
//     tuples(Faulty, Heights, Rounds, Values, Steps)
//     .map(((p, h, r, v, s)) => { src: p, height: h, round: r, id: v, step: s })

val AllFaultyProposals : Set[Proposal_t] =
    tuples(Faulty, Heights, Rounds, Values, RoundsOrNil)
    .map(t => { src: t._1, height: t._2, round: t._3, proposal: t._4, validRound: t._5 })


// Global State
var system : Address_t -> NodeState
var propBuffer : Address_t -> Set[Proposal_t]
var voteBuffer : Address_t -> Set[Vote_t]
var _hist: { validator: Address_t, input: DriverInput, output: ConsensusOutput }

val ConsensusOutputInv = consensusOutputs.union(Set(defaultOutput.name)).contains(_hist.output.name) 


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
    _hist' = { validator: "INIT", input: defaultInput, output: defaultOutput }
}

// Put the proposal into the buffers of all validators
pure def sendProposal (buffer: Address_t -> Set[Proposal_t], prop: Proposal_t) : Address_t -> Set[Proposal_t] = {
    buffer.keys().mapBy(x => 
     { buffer.get(x).union(Set(prop)) })
}

// Put the vote into the inbuffers of all validators
pure def sendVote (sys: Address_t -> Set[Vote_t], vote: Vote_t) : Address_t -> Set[Vote_t] = {
    sys.keys().mapBy(x => 
     { ...sys.get(x).union(Set(vote)) })
}

// Record that a timeout has started at node v
pure def startTimeout (sys: Address_t -> NodeState, v: Address_t, toName: str) : Address_t -> NodeState = {
    sys.put(v, { ...sys.get(v), timeout: sys.get(v).timeout.union(Set(
        {(toName, sys.get(v).es.cs.height, sys.get(v).es.cs.round)}
    ))})
}

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
        if (res._2.name == "proposal") all {
            // TODO: do we need to start a timeout on this result, that is, when we are proposer and
            // send a proposal?
            propBuffer' = sendProposal(propBuffer, res._2.proposal), 
            voteBuffer' = voteBuffer,
            system' = sys,
        }
        else if (res._2.name == "votemessage") all {
            propBuffer' = propBuffer,
            voteBuffer' = sendVote(voteBuffer, res._2.voteMessage), 
            system' = sys,
        }
        else if (res._2.name == "timeout") all {
            propBuffer' = propBuffer,
            voteBuffer' = voteBuffer,
            system' = startTimeout(sys, v, res._2.timeout),
        }
        else if (res._2.name == "skipRound") all {
            propBuffer' = propBuffer,
            voteBuffer' = voteBuffer,
            //skipRound should never leave the driver
            system' = sys,
        }
        else all {
            propBuffer' = propBuffer,
            voteBuffer' = voteBuffer,
            system' = sys,
        },
        _hist' = { validator: v, input: input._2, output: res._2 }
//         _histSimple' = (v, input._2.name, res._2.name)
    }
}

action setNextValueToPropose(v: Address_t, value: Value_t) : bool = all {
    val res = driver(system.get(v).es, { ...defaultInput, name: "SetNextProposedValue", nextValueToPropose: value})
    val newNS = { ...system.get(v), es: res._1}
    system' = system.put(v, newNS),
    _hist' = _hist,
    propBuffer' = propBuffer,
    voteBuffer' = voteBuffer,
}

action deliverProposal(v: Address_t, p: Proposal_t) : bool = all {
    propBuffer.get(v).union(AllFaultyProposals).contains(p), // the proposal must be sent or from a faulty node
    propBuffer' = propBuffer.put(v, propBuffer.get(v).exclude(Set(p))),
    system' = system.put(v, { ...system.get(v), incomingProposals: system.get(v).incomingProposals.union(Set(p)) }),
    _hist' = _hist,
    voteBuffer' = voteBuffer, 
}

action deliverVote(v: Address_t, vote: Vote_t) : bool = all {
    voteBuffer.get(v).union(AllFaultyVotes).contains(vote), // the vote must be sent or from a faulty node
    voteBuffer' = voteBuffer.put(v, voteBuffer.get(v).exclude(Set(vote))),
    system' = system.put(v, { ...system.get(v), incomingVotes: system.get(v).incomingVotes.union(Set(vote)) }),
    _hist' = _hist,
    propBuffer' = propBuffer, 
}

// deliver a message. Take it from the network buffer of from the faulty set
//  and put it into the incoming sets
action deliver(v: Address_t) : bool = any {
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
