// -*- mode: Bluespec; -*-

// This contains 
// 1. asynchronous message transfer semantics
// 2. parameterization with too many faulty nodes 
// 3. a test where disagreement fails
// TODO: cleanup. 1. needs to be re-usable also for good parameters 

module statemachineAsync {

import executor.* from "./executor"
import consensus.* from "./consensus"
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

val AllFaultyVotes : Set[VoteMsg_t] = 
    tuples(Faulty, Heights, Rounds, Values, Steps)
    .map((p, h, r, v, s) => { src: p, height: h, round: r, id: v, step: s })

val AllFaultyProposals : Set[ProposeMsg_t] =
    tuples(Faulty, Heights, Rounds, Values, RoundsOrNil)
    .map((p, h, r, v, vr) => { src: p, height: h, round: r, proposal: v, validRound: vr })


// Global State
var system : Address_t -> NodeState
var propBuffer : Address_t -> Set[ProposeMsg_t]
var voteBuffer : Address_t -> Set[VoteMsg_t]
var _hist: (str, ExecutorInput, ConsensusOutput)

action unchangedAll = all {
    system' = system,
    propBuffer' = propBuffer,
    voteBuffer' = voteBuffer,
    _hist' = _hist
}

val AgreementInv = tuples(Correct, Correct).forall((p,q) => 
    (system.get(p).es.chain != List() and system.get(q).es.chain != List())
        implies
            system.get(p).es.chain[0] == system.get(q).es.chain[0])

// Actions
action init = all {
    system' = Correct.mapBy(v => initNode(v, validatorSet)),
    propBuffer' = Correct.mapBy(v => Set()),
    voteBuffer' = Correct.mapBy(v => Set()),
    _hist' = ("INIT", defaultInput, defaultResult)
}

// Put the proposal into the buffers of all validators
pure def sendProposal (buffer: Address_t -> Set[ProposeMsg_t], prop: ProposeMsg_t) : Address_t -> Set[ProposeMsg_t] = {
    buffer.keys().mapBy(x => 
     { buffer.get(x).union(Set(prop)) })
}

// Put the vote into the inbuffers of all validators
pure def sendVote (sys: Address_t -> Set[VoteMsg_t], vote: VoteMsg_t) : Address_t -> Set[VoteMsg_t] = {
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
    val input = system.get(v).nextAction() // TODO: nextAction could go within executor boundary
    // remove action from v
    val sys1 = system.put(v, input._1)
    // call executor
    val res = executor(sys1.get(v).es, input._2)
    all {
        // update v's state after the step
        val sys = sys1.put(v, { ...sys1.get(v), es: res._1})
        // do networking
        if (res._2.name == "proposal") all {
            propBuffer' = sendProposal(propBuffer, res._2.proposal), // TODO: this is immediate
            voteBuffer' = voteBuffer,
            system' = sys,
        }
        else if (res._2.name == "votemessage") all {
            propBuffer' = propBuffer,
            voteBuffer' = sendVote(voteBuffer, res._2.voteMessage), // TODO: this is immediate
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
            //skipRound should never leave the executor
            system' = sys,
        }
        else all {
            propBuffer' = propBuffer,
            voteBuffer' = voteBuffer,
            system' = sys,
        },
        _hist' = (v, input._2, res._2)
//         _histSimple' = (v, input._2.name, res._2.name)
    }
}

action setNextValueToPropose(v: Address_t, value: Value_t) : bool = all {
    val res = executor(system.get(v).es, { ...defaultInput, name: "SetNextProposedValue", nextValueToPropose: value})
    val newNS = { ...system.get(v), es: res._1}
    system' = system.put(v, newNS),
    _hist' = _hist,
    propBuffer' = propBuffer,
    voteBuffer' = voteBuffer,
}

action deliverProposal(v: Address_t, p: ProposeMsg_t) : bool = all {
    propBuffer.get(v).union(AllFaultyProposals).contains(p), // the proposal must be sent or from a faulty node
    propBuffer' = propBuffer.put(v, propBuffer.get(v).exclude(Set(p))),
    system' = system.put(v, { ...system.get(v), incomingProposals: system.get(v).incomingProposals.union(Set(p)) }),
    _hist' = _hist,
    voteBuffer' = voteBuffer, 
}

action deliverVote(v: Address_t, vote: VoteMsg_t) : bool = all {
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