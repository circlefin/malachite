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
export types.*
import driver.* from "./driver"
export driver.* 
import consensus.* from "./consensus"
export consensus.* 
import votekeeper.* from "./votekeeper"
import extraSpells.* from "./spells/extra"

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
var certBuffer : Address -> Set[Set[Vote]]
var voteBuffer : Address -> Set[Vote]
var _hist: { validator: Address, input: DriverInput, output: ConsensusOutput }



action unchangedAll = all {
    system' = system,
    propBuffer' = propBuffer,
    voteBuffer' = voteBuffer,
    certBuffer' = certBuffer,
    _hist' = _hist
}

/// Any two processes agree in the consensus instances in which they both have already decided
val AgreementInv = tuples(Correct, Correct).forall(p => 
    val numDecidedInstances = min(  system.get(p._1).es.chain.length(),
                                    system.get(p._2).es.chain.length())
    to(0, numDecidedInstances-1).forall(i =>
            system.get(p._1).es.chain[i] == system.get(p._2).es.chain[i])
)

// Actions
action init = all {
    system' = Correct.mapBy(v => initNode(v, validatorSet, 0)),
    propBuffer' = Correct.mapBy(v => Set()),
    voteBuffer' = Correct.mapBy(v => Set()),
    certBuffer' = Correct.mapBy(v => Set()),
    _hist' = { validator: "INIT", input: NoDInput, output: NoConsensusOutput }
}



// Put the proposal into the buffers of all validators
pure def sendProposal(buffer: Address -> Set[Proposal], prop: Proposal): Address -> Set[Proposal] =
    buffer.keys().mapBy(x => { buffer.get(x).union(Set(prop)) })

// Put the vote into the inbuffers of all validators
pure def sendVote(buffer: Address -> Set[Vote], vote: Vote): Address -> Set[Vote] =
    buffer.keys().mapBy(x => { ...buffer.get(x).union(Set(vote)) })

// Put the certificate into the inbuffers of all validators
pure def sendCertificate(buffer : Address -> Set[Set[Vote]], cert: Certificate): Address -> Set[Set[Vote]] =
    match cert {
        | NoCertificate => buffer
        | Polka(c) => buffer.keys().mapBy(x => { ...buffer.get(x).union(Set(c)) })
        | Commit(c) => buffer.keys().mapBy(x => { ...buffer.get(x).union(Set(c)) })
    }

// Record that a timeout has started at node v
// As we do set union here, there is no need to check whether this happens the first time
pure def startTimeout(sys: Address -> NodeState, v: Address, timeout: Timeout) : Address -> NodeState =
    val newTimeout = (timeout, sys.get(v).es.cs.height, sys.get(v).es.cs.round)
    sys.put(v, { ...sys.get(v), timeouts: sys.get(v).timeouts.union(Set(newTimeout)) })

// record a get value request
pure def addGetValueRequest(sys: Address -> NodeState, v: Address, h: Height, r: Round) : Address -> NodeState =
    sys.put(v, { ...sys.get(v), getValueRequests: sys.get(v).getValueRequests.union(Set((h,r)))})

action doStep(v, next_action) = {
    val node_state = next_action._1
    val driver_input = next_action._2
    // remove action from v
    val sys1 = system.put(v, node_state)
    // call driver
    val res = driver(sys1.get(v).es, driver_input)
    val driver_state = res.ds
    val consensus_output = res.out
    all {
        // update v's state after the step
        val sys = sys1.put(v, { ...sys1.get(v), es: driver_state})
        // do networking
        match consensus_output {
            | ProposalOutput(proposal) => all {
                propBuffer' = sendProposal(propBuffer, proposal), 
                voteBuffer' = voteBuffer,
                // if there has been the GetValue flow before, the timeout is already started
                // but since it is added to a set it is no harm in calling startTimeout again
                // in the model
                system' = startTimeout(sys, v, ProposeTimeout),
            }
            | GetValueAndScheduleTimeoutOutput(height_round_timeout) => all{
                propBuffer' = propBuffer, 
                voteBuffer' = voteBuffer,
                // record the value request and start the propose timeout
                val sysVal = addGetValueRequest(sys, v, height_round_timeout._1, height_round_timeout._2)
                system' = startTimeout(sysVal, v, height_round_timeout._3),
            }
            | VoteOutput(vote) => all {
                propBuffer' = propBuffer,
                voteBuffer' = sendVote(voteBuffer, vote), 
                system' = sys,
            }
            | TimeoutOutput(round_timeout) => 
                val round = round_timeout._1 // not used
                val timeout = round_timeout._2
                all {
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

        // we make a certificate available to the others. In the implementation (and potentially
        // a more high-level node spec) we will need to be more clever about this, e.g., only
        // send the certificate if I am in a future round and the proposer
        certBuffer' = sendCertificate(certBuffer, res.certificate),

        _hist' = { validator: v, input: driver_input, output: consensus_output }
//         _histSimple' = (v, input._2.name, res._2.name)
    }
}


action valTimeout(v: Address) : bool = {
    if (system.get(v).existsTimeout())
        val next_action = system.get(v).pickTimeout()
        doStep(v, next_action)
    else
       unchangedAll 
}

action valStep(v: Address) : bool = {
    // Pick next action to execute, excluding timeouts
    val next_action = system.get(v).nextAction() 
    doStep(v, next_action)
}


action setNextValueToPropose(v: Address, value: NonNilValue) : bool = all {
    val newNS = setValue(system.get(v), value)
    system' = system.put(v, newNS),
    _hist' = _hist,
    propBuffer' = propBuffer,
    voteBuffer' = voteBuffer,
    certBuffer' = certBuffer,
}


action deliverProposal(v: Address, p: Proposal) : bool = all {
    propBuffer.get(v).union(AllFaultyProposals).contains(p), // the proposal must be sent or from a faulty node
    propBuffer' = propBuffer.put(v, propBuffer.get(v).exclude(Set(p))),
    system' = system.put(v, { ...system.get(v), incomingProposals: system.get(v).incomingProposals.union(Set(p)) }),
    _hist' = _hist,
    voteBuffer' = voteBuffer, 
    certBuffer' = certBuffer,
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
    certBuffer' = certBuffer,
}

action deliverCertificate(v: Address, c: Set[Vote]) : bool = all {
    certBuffer.get(v).contains(c), // the cert must be sent 
    certBuffer' = certBuffer.put(v, certBuffer.get(v).exclude(Set(c))),
    system' = system.put(v, { ...system.get(v), incomingCertificates: system.get(v).incomingCertificates.union(Set(c)) }),
    _hist' = _hist,
    propBuffer' = propBuffer,    
    voteBuffer' = voteBuffer, 
}

action deliverSomeCertificate(v: Address) : bool = all {
    certBuffer.get(v).size() > 0,
    nondet c = oneOf(certBuffer.get(v))
    deliverCertificate(v, c)
}


// deliver a message or a certificate. 
// Take it from the network buffer of from the faulty set and put it into the incoming sets
action deliver(v: Address) : bool = any {
    val props = propBuffer.get(v).union(AllFaultyProposals)
    all{
        props.size() > 0,
        nondet prop = oneOf(props)
        deliverProposal(v, prop),
    },
    val votes = voteBuffer.get(v).union(AllFaultyVotes)
    all {
        votes.size() > 0,
        nondet vote = oneOf(votes)
        deliverVote(v, vote),
    },
    deliverSomeCertificate(v),  // we might do fault injection here. But faulty nodes cannot
                                // totally make up certificates. They cannot forge messages to look 
                                // like being sent by correct nodes
}

action step = {
    nondet v = oneOf(Correct)
    nondet value = oneOf(Values)
    any {
        valStep(v),
        valTimeout(v), // comment out for runs where no timeout ever expires
        deliver(v),
        setNextValueToPropose(v, value),
    }
}

/// environment sends the node to the next height.
/// checks that we have all the previous blocks in the store.
action newHeightAction(v, valset, h) = all {
    system.get(v).es.chain.length() >= system.get(v).es.cs.height,
    val ns = system.get(v).with("es", initDriver(v, valset, h, system.get(v).es.chain))
    system' = system.put(v, ns), 
    _hist' = _hist,
    propBuffer' = propBuffer,
    voteBuffer' = voteBuffer,
    certBuffer' = certBuffer,
}

/// retreat workaround. TODO: check with Gabriela about Quint flattening
/// the code should be situation in sync.qnt at action syncStepClient(v)
action putSyncOutputIntoNode(v, syncOut) = all{
        system' = match syncOut {
            // Put the sync output into the consensus engine
            | SOCertificate(cert) => system.put(v, {... system.get(v), 
                    incomingSyncCertificates: 
                        system.get(v).incomingSyncCertificates.union(Set(cert))})
            | SOValue(prop) => system.put(v, {... system.get(v), 
                    incomingSyncProposals: 
                        system.get(v).incomingSyncProposals.union(Set(prop))})
            | SOValueStoreEntry(b) => system.put(v, {... system.get(v), 
                    incomingSyncProposals: 
                        system.get(v).incomingSyncProposals.union(Set(b.decision)),
                    incomingSyncCertificates: 
                        system.get(v).incomingSyncCertificates.union(Set(b.commit))})
            | SONoOutput => system
        },
        propBuffer' = propBuffer,
        voteBuffer' = voteBuffer,
        certBuffer' = certBuffer,
        _hist' = _hist,
}

}
