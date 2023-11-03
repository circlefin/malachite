// -*- mode: Bluespec; -*-

module executor {

import consensus.* from "./consensus"
import voteBookkeeper.* from "./voteBookkeeper"

type ExecutorState = {
    bk : Bookkeeper,
    cs : ConsensusState,
    proposals: Set[ProposeMsg_t],
    valset: Address_t -> int,
    executedEvents: Set[(Event, Height_t, Round_t)],
    pendingEvents: Set[(Event, Height_t, Round_t)],
    started: bool,
    applyvotesResult: ExecutorEvent, //debug TODO!!!
    chain : List[Value_t],
}

type NodeState = {
    es: ExecutorState,
    timeout: Set[(Timeout_t, Height_t, Round_t)],
    incomingVotes: Set[VoteMsg_t],
    incomingProposals: Set[ProposeMsg_t],
}

type ExecutorInput = {
    name: str,
    proposal: ProposeMsg_t,
    vote: VoteMsg_t,
    timeout: str,
    event: Event
}

val defaultInput: ExecutorInput = {name: "",
    proposal: { src: "", height: 0, round: 0, proposal: "", validRound: 0 },
    vote: { src: "", height: 0, round: 0, step: "", id: "", },
    timeout: "",
    event: defaultEvent}

// this is to match the type from the bookkeeper. When we add heights there we should
// unify
pure def toVote (v: VoteMsg_t) : Vote = 
    { typ: v.step, round: v.round, value: v.id, address: v.src }

val defaultEvent : Event = { name : "", height : 0, round: 0, value: "", vr: 0 }    

/*
TODO:  encode the following decision tree

proposal
    invalid -> consensus event
    valid:  BK list of events 
            generate all Consensus events C
            feed all events to Consensus

Precommit
    BK applyVote
    (see whether we can make (A) 2f+1 Any for current or 
                             (B) 2f+1 plus proposal for current)
    -> feed all events to Consensus

Prevote
    BK applyVote 
    For current round PolkaVal -> PolkaAny and PolkaNil -> PolkaAny: list of Events
    check proposal: list of Consensus evens (considering line 28)
    -> feed all events to Consensus
*/

val emptyProposal : ProposeMsg_t=
    { src: "none", height: 0, round: 0, proposal: "none", validRound: 0 }

val emptyVote = 
    { src: "none", height: 0, round: 0, step: "None", id: "None" }

//
// Interface to app and/or mempool (Proposer, getValue, valid)
// 
// Rotating coordinator. We can do something more clever actually using the valset
pure def Proposer(valset: Address_t -> int, height: Height_t, round: Round_t) : Address_t = {
    val v = valset
    val prop = (height * round + 1) % 4
    if (prop == 0) "v1"
    else if  (prop == 1) "v2"
    else if  (prop == 2) "v3"
    else "v4"
}

pure def getValue(cs: ConsensusState, round:int) : Value_t = "a block"

pure def valid(p: ProposeMsg_t) :bool = {
    // for simulation, if the value is "invalid", so is the proposal
    // if this is to become "non-deterministic", we must push it
    // and its use into the state machine
    p.proposal != "invalid"
}

// efficient hashing function
pure def id(v) = v 


// check whether the event has been already sent to consensus. If not, do so.
pure def callConsensus (es: ExecutorState, bk: Bookkeeper, ev: Event) : (ExecutorState, ConsensusOutput) = {
    // check whether we already executed the event already
    if (es.executedEvents.contains((ev, es.cs.height, es.cs.round)))
        ({ ...es, bk:bk, cs: es.cs}, defaultResult)
    else
        val res = consensus(es.cs, ev)
        ({ ...es, bk: bk, cs: res.cs, 
        executedEvents: es.executedEvents.union(Set((ev, res.cs.height, res.cs.round)))}, 
        res.out)
}

// We do this if the executor receives a Precommit
pure def Precommit (es: ExecutorState, input: ExecutorInput, eev: ExecutorEvent) : (ExecutorState, ConsensusOutput) = {
    if (eev.name == "PrecommitValue")
            if (es.proposals.exists(x => eev.round == x.round and eev.value == id(x.proposal))) {
                callConsensus(es, es.bk, {  name : "ProposalAndCommitAndValid",
                                            height : input.vote.height,
                                            round: input.vote.round,
                                            value: input.vote.id,
                                            vr: -1})
            }
            else {
                if (eev.round == es.cs.round) {
                    callConsensus(es, es.bk,  { name: "PrecommitAny",
                                                height: input.vote.height,
                                                round: input.vote.round,
                                                value: input.vote.id,
                                                vr: -1})   
                }
                else if (eev.round > es.cs.round) {
                    // if it is for a future round I can trigger skipround
                    // TODO: should we really do this. It is dead code as the f+1 event already happened
                    callConsensus(es, es.bk,  { name: "RoundSkip",
                                                height: input.vote.height,
                                                round: input.vote.round,
                                                value: input.vote.id,
                                                vr: -1})     
                }  
                else {
                    // messages from past round -> ignore
                    (es, defaultResult)
                }    
            }
    else if (eev.name == "PrecommitAny" and eev.round == es.cs.round) {
                callConsensus(es, es.bk,  { name: "PrecommitAny",
                                            height: input.vote.height,
                                            round: input.vote.round,
                                            value: input.vote.id,
                                            vr: -1})    
    }
    else
        // none of the supported Precommit events. Do nothing
        (es, defaultResult)  
}


// We do this if the executor receives a Prevote
pure def Prevote (es: ExecutorState, input: ExecutorInput,  eev: ExecutorEvent) : (ExecutorState, ConsensusOutput) = {
    // TODO: events do not have heights now.
    // TODO: Polka implications missing.
    if (eev.name == "PolkaValue")
        if (eev.round < es.cs.round and
            es.proposals.exists(p => p.round == es.cs.round and
            eev.round == p.validRound and id(p.proposal) == eev.value)) 
                callConsensus(es, es.bk, {  name: "ProposalAndPolkaPreviousAndValid",
                                            height: es.cs.height,
                                            round: es.cs.round,
                                            value: eev.value,
                                            vr: eev.round})   
                                            // TODO: the value should come from the proposal   
        else if (eev.round == es.cs.round and
                es.proposals.exists(p => p.round == es.cs.round and
                id(p.proposal) == eev.value)) 
                callConsensus(es, es.bk, {  name: "ProposalAndPolkaAndValid",
                                            height: es.cs.height,
                                            round: es.cs.round,
                                            value: eev.value,
                                            vr: eev.round})   
        else
            // we don't have a matching proposal so we do nothing
            // TODO: we might check whether it is for a future round and jump
            (es, defaultResult)
    else if (eev.name == "PolkaAny")
        if (eev.round == es.cs.round)
                // call consensus and remember that we did it
                callConsensus(es, es.bk, {  name: "PolkaAny",
                                            height: es.cs.height,
                                            round: es.cs.round,
                                            value: eev.value,
                                            vr: eev.round})  
        else
            // TODO: we might check whether it is for a future round and jump
            (es, defaultResult)
    else if (eev.name == "PolkaNil" and eev.round == es.cs.round)
        callConsensus(es, es.bk,  { name: "PolkaNil",
                                    height: es.cs.height,
                                    round: es.cs.round,
                                    value: eev.value,
                                    vr: eev.round})  
    else
        (es, defaultResult)
}

// We do this if a timeout expires at the executor
pure def Timeout (es: ExecutorState, input: ExecutorInput) : (ExecutorState, ConsensusOutput) = {
    // TODO: We assume that the timeout event is always for the current round. If this is not
    // the case, we need to encode it in the input to which round the timeout belongs
    val event: Event = {name: input.timeout, 
                        height: es.cs.height,
                        round: es.cs.round, 
                        value: "", 
                        vr: 0}
    callConsensus(es, es.bk, event)
}

// We do this if the executor receives a proposal
pure def ProposalMsg (es: ExecutorState, input: ExecutorInput) : (ExecutorState, ConsensusOutput) = {
    val newES = { ...es, proposals: es.proposals.union(Set(input.proposal))}
    if (input.proposal.src != Proposer(es.valset, es.cs.height, es.cs.round))
        // keep ES (don't use newES here), that is, drop proposal
        (es, defaultResult)
    else if (not(valid(input.proposal))) {
        // keep ES (don't use newES here), that is, drop proposal
        if (es.cs.step == "propose")
            if (checkThreshold(es.bk, es.cs.round, "Prevote", {name: "PolkaValue", value: id(input.proposal.proposal)}))
                val event: Event = {name: "ProposalAndPolkaAndInValid", 
                                    height: es.cs.height,
                                    round: es.cs.round, 
                                    value: id(input.proposal.proposal), 
                                    vr: 0}
                callConsensus(es, es.bk, event)
            else 
                val event: Event = {name: "ProposalInvalid", 
                                    height: es.cs.height,
                                    round: es.cs.round, 
                                    value: id(input.proposal.proposal), 
                                    vr: 0}
                callConsensus(es, es.bk, event)
        else 
            (es, defaultResult)
    }
    else { // proposal valid and from proposer
        val receivedPolkaValidRoundVal = checkThreshold(newES.bk,   
                                                        input.proposal.validRound, 
                                                        "Prevote", 
                                                        {name: "PolkaValue", 
                                                        value: id(input.proposal.proposal)})
        val receivedPolkaCurrentVal = checkThreshold(   newES.bk, 
                                                        newES.cs.round, 
                                                        "Prevote", 
                                                        {name: "PolkaValue", 
                                                        value: id(input.proposal.proposal)})
        val receivedCommitCurrentVal = checkThreshold(  newES.bk, 
                                                        newES.cs.round, 
                                                        "Precommit", 
                                                        {name: "PrecommitValue", 
                                                        value: id(input.proposal.proposal)})
        if (newES.cs.step == "propose") 
            if (input.proposal.validRound == -1)
                if (receivedPolkaCurrentVal)
                    callConsensus(  newES, 
                                    newES.bk,  
                                    {   name: "ProposalAndPolkaAndValid",
                                        height: newES.cs.height,
                                        round: newES.cs.round,
                                        value: id(input.proposal.proposal),
                                        vr: input.proposal.round})   
                else
                    callConsensus(  newES, 
                                    newES.bk,  
                                    {   name: "Proposal",
                                        height: newES.cs.height,
                                        round: newES.cs.round,
                                        value: id(input.proposal.proposal),
                                        vr: input.proposal.round})    
            else
                if (receivedPolkaValidRoundVal)
                    callConsensus(  newES, 
                                    newES.bk,  
                                    {   name: "ProposalAndPolkaPreviousAndValid",
                                        height: newES.cs.height,
                                        round: newES.cs.round,
                                        value: id(input.proposal.proposal),
                                        vr: input.proposal.round})  
                else  
                    (newES, defaultResult)
        else if (newES.cs.step == "Prevote" or newES.cs.step == "Precommit") 
            if (receivedCommitCurrentVal)
                // TODO: here we need to call both, Commit and Polka. 
                // We do commit and append pola to pending
                val pend = ( {  name: "ProposalAndPolkaAndValid",
                                height: newES.cs.height,
                                round: newES.cs.round,
                                value: id(input.proposal.proposal),
                                vr: input.proposal.round},
                            newES.cs.height,
                            newES.cs.round)
                callConsensus(  { ...newES, pendingEvents: newES.pendingEvents.union(Set(pend))}, 
                                newES.bk,  
                                {   name: "ProposalAndCommitAndValid",
                                    height: newES.cs.height,
                                    round: newES.cs.round,
                                    value: id(input.proposal.proposal),
                                    vr: input.proposal.round})  
            
            else if (receivedPolkaCurrentVal)
                callConsensus( newES , 
                newES.bk, { name: "ProposalAndPolkaAndValid",
                            height: newES.cs.height,
                            round: newES.cs.round,
                            value: id(input.proposal.proposal),
                            vr: input.proposal.round})  
            else 
                (newES, defaultResult)
        else
            (newES, defaultResult)
    }
}

// We do this when we need to jump to a new round
pure def skip (es: ExecutorState, r: int) : (ExecutorState, ConsensusOutput) = {
    // line 15
    val prop =  if (es.cs.validValue != "nil") es.cs.validValue
                else getValue(es.cs, r)
    if (Proposer(es.valset, es.cs.height, es.cs.round + 1) == es.cs.p)
        callConsensus(es, es.bk,  { name: "NewRoundProposer",
                                    height: es.cs.height,
                                    round: r,
                                    // what value?
                                    value: prop,
                                    vr: es.cs.validRound})  
    else 
        callConsensus(es, es.bk,  { name: "NewRound",
                                    height: es.cs.height,
                                    round: r,
                                    value: "",
                                    vr: es.cs.validRound})  
    // what do we do now in the new round? Shouldn't we look whether we can build an event.
    // TODO: compute pending events.
}

// We do this when we have decided
pure def decided (es: ExecutorState, res: ConsensusOutput) : (ExecutorState, ConsensusOutput) = {
    // here we call consensus to set a new height, that is, to initialize the state machine
    // and then we call skip to start round 0
    // TODO: one can put res.decided into the blockstore

/*  // The following should be used to get to the next height. For now this
    // function does nothing      
    val s1 = callConsensus(es, es.bk, {name : "NewHeight",
                                    height : es.cs.height + 1,
                                    round: -1,
                                    value: "",
                                    vr: es.cs.validRound})  
    skip (s1._1, 0) 
*/
    ({ ...es, chain: es.chain.append(res.decided) } , res)
}

// take input out of pending events and then call consensus with that event
// We do this when the executor is asked to work on pending events
pure def PendingEvent (es: ExecutorState, input: ExecutorInput) : (ExecutorState, ConsensusOutput) = {
    val newState = { ...es, pendingEvents: es.pendingEvents.exclude(Set((input.event, es.cs.height, es.cs.round)))}
    callConsensus(newState, es.bk, input.event)
}


/* *********************************************************
 * Main entry point
 * ********************************************************/

pure def executor (es: ExecutorState, input: ExecutorInput) : (ExecutorState, ConsensusOutput) = {
    // TODO: shall we check whether the sender actually is in the validator set
    if (input.name == "proposal") {
        val cons_res = ProposalMsg(es, input)
        if (cons_res._2.name == "decided")
            decided (cons_res._1, cons_res._2)
        else
            cons_res
    }
    else if (input.name == "votemessage" and input.vote.step == "Precommit") {
        val res = applyVote(es.bk, toVote(input.vote), es.valset.get(input.vote.src))
        val newES = { ...es, bk: res.bookkeeper, applyvotesResult: res.event}
        // only a commit event can come here. 
        val cons_res = Precommit(newES, input, res.event) 
        if (cons_res._2.name == "decided")
            decided (cons_res._1, cons_res._2)
        else
            cons_res
    }
    else if (input.name == "votemessage" and input.vote.step == "Prevote") {
        val res = applyVote(es.bk, toVote(input.vote), es.valset.get(input.vote.src))
        val newES = { ...es, bk: res.bookkeeper, applyvotesResult: res.event}
        // only a commit event can come here. 
        Prevote(newES, input, res.event) 
    }
    else if (input.name == "timeout") {
        val res = Timeout(es, input)
        // result should be vote or skip
        if (res._2.name == "skipRound")
            skip (res._1, res._2.skipRound)
        else
            res
    }
    else if (input.name == "start") {
        //
        val new = { ...es, started: true}
        skip (new, 0)
    }
    else if (input.name == "pending") {
        PendingEvent(es, input)
    }
    else
        (es, defaultResult)
}

// This is a simple function that figures out in what external events (messages,
// timeouts, etc.) the node should act.
// currently this is linked in via the state machine. But we can move it into
// the functional layer/
pure def nextAction (state: NodeState) : (NodeState, ExecutorInput) = {
    if (not(state.es.started))
        (state, { ...defaultInput, name: "start" })

    else if (state.incomingProposals != Set())
        // pick proposal, remove it from incoming
        // val prop = state.incomingProposals.chooseSome()
        val prop = state.incomingProposals.fold (emptyProposal, (sum, y) => y)
        val newstate = { ...state, incomingProposals: state.incomingProposals.exclude(Set(prop))}
        (newstate, { ...defaultInput, name: "proposal", proposal: prop})

    else if (state.incomingVotes != Set())
        // pick vote, remove it from incoming
        // val vote = state.incomingVotes.chooseSome()
        val vote = state.incomingVotes.fold(emptyVote, (sum, y) => y)
        val newstate = { ...state, incomingVotes: state.incomingVotes.exclude(Set(vote))}
        (newstate, { ...defaultInput, name: "votemessage", vote: vote})

    else if (state.timeout.exists(x => x._2 == state.es.cs.height and 
        x._3 == state.es.cs.round))
        // pick timeout, remove it from incoming
        val timeouts = state.timeout.filter(x => x._2 == state.es.cs.height and 
            x._3 == state.es.cs.round)
        // val timeout = timeouts.chooseSome()
        val timeout = timeouts.fold(("", 0, 0), (sum, y) => y)
        val newstate = { ...state, timeout: state.timeout.exclude(Set(timeout))}
        (newstate, { ...defaultInput, name: "timeout", timeout: timeout._1})

    else if (state.es.pendingEvents != Set())
        // this might be cheating as we look into the "es"
        (state, { ...defaultInput, name: "pending" })

    else
        (state, defaultInput)
}

// This function can be used to control test runs better.
pure def nextActionCommand (state: NodeState, command: str) : (NodeState, ExecutorInput) = {
    if (command == "start" and not(state.es.started))
        (state, { ...defaultInput, name: "start" })

    else if (command == "proposal" and state.incomingProposals != Set())
        // pick proposal, remove it from incoming
        // val prop = state.incomingProposals.chooseSome()
        val prop = state.incomingProposals.fold (emptyProposal, (sum, y) => y)
        val newstate = { ...state, incomingProposals: state.incomingProposals.exclude(Set(prop))}
        (newstate, { ...defaultInput, name: "proposal", proposal: prop})

    else if (command == "vote" and state.incomingVotes != Set())
        // pick vote, remove it from incoming
        // val vote = state.incomingVotes.chooseSome()
        val vote = state.incomingVotes.fold(emptyVote, (sum, y) => y)
        val newstate = { ...state, incomingVotes: state.incomingVotes.exclude(Set(vote))}
        (newstate, { ...defaultInput, name: "votemessage", vote: vote})

    else if (command == "timeout" and state.timeout.exists(x => x._2 == state.es.cs.height and 
        x._3 == state.es.cs.round))
        // pick timeout, remove it from incoming
        val timeouts = state.timeout.filter(x => x._2 == state.es.cs.height and 
            x._3 == state.es.cs.round)
        // val timeout = timeouts.chooseSome()
        val timeout = timeouts.fold(("", 0, 0), (sum, y) => y)
        val newstate = { ...state, timeout: state.timeout.exclude(Set(timeout))}
        (newstate, { ...defaultInput, name: "timeout", timeout: timeout._1})

    else if (command == "pending" and state.es.pendingEvents != Set())
        // this might be cheating as we look into the "es"
        (state, { ...defaultInput, name: "pending" })

    else
        (state, defaultInput)
}

}