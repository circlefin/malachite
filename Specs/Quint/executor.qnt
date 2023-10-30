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

//this is a hack. We need to understand duplicate votes eventually
pure def toVote (v: VoteMsg_t) : Vote = 
    { typ: v.step, round: v.round, value: v.id, address: v.src }

val defaultEvent : Event = { name : "", height : 0, round: 0, value: "", vr: 0 }    

/*
TODO:  encode the following decision tree

proposal
    invalid -> consensus event
    valid:  BK get list of events 
            generate all Consensus events C
            feed all events to Consensus

precommit
    BK applyVote
    (see whether we can make (A) 2f+1 Any for current or 
                             (B) 2f+1 plus proposal for current)
    -> feed all events to Consensus

prevote
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
// Rotating coordinator. We can do something more clever
pure def Proposer(height: Height_t, round: Round_t) : Address_t = {
    val prop = (height * round + 1) % 4
    if (prop == 0) "v1"
    else if  (prop == 1) "v2"
    else if  (prop == 2) "v3"
    else "v4"
}
pure def getValue(cs: ConsensusState, round:int) : Value_t = "empty"
pure def valid(v) = true

// efficient hashing function
pure def id(v) = v 


// check whether the event has been already sent to consensus. If not, do so.
pure def callConsensus (es: ExecutorState, bk: Bookkeeper, ev: Event) : (ExecutorState, Result) = {
    if (es.executedEvents.contains((ev, es.cs.height, es.cs.round)))
        ({ ...es, bk:bk, cs: es.cs}, defaultResult)
    else
        val res = consensus(es.cs, ev)
        ({ ...es, bk: bk, cs: res._1, executedEvents: es.executedEvents.union(Set((ev, res._1.height, res._1.round)))}, res._2)
}

// We do this if the executor receives a precommit
pure def Precommit (es: ExecutorState, input: ExecutorInput, bkr: BKResult) : (ExecutorState, Result) = {
    if (bkr.event.name == "PrecommitValue")
            if (es.proposals.exists(x => bkr.event.round == x.round and bkr.event.value == x.proposal)) {
                callConsensus(es, bkr.bookkeeper, {name : "ProposalAndCommitAndValid",
                                    height : input.vote.height,
                                    round: input.vote.round,
                                    value: input.vote.id,
                                    vr: -1})
            }
            else {
                if (bkr.event.round == es.cs.round) {
                    callConsensus(es, bkr.bookkeeper,  {name : "PrecommitAny",
                                    height : input.vote.height,
                                    round: input.vote.round,
                                    value: input.vote.id,
                                    vr: -1})   
                }
                else if (bkr.event.round > es.cs.round) {
                    // if it is for a future round I can trigger skipround
                    // TODO: should we really do this. It is dead code as the f+1 event already happened
                    callConsensus(es, bkr.bookkeeper,  {name : "RoundSkip",
                                    height : input.vote.height,
                                    round: input.vote.round,
                                    value: input.vote.id,
                                    vr: -1})     
                }  
                else {
                    // messages from past round -> ignore
                    (es, defaultResult)
                }    
            }
    else if (bkr.event.name == "PrecommitAny" and bkr.event.round == es.cs.round) {
                callConsensus(es, bkr.bookkeeper,  {name : "PrecommitAny",
                                    height : input.vote.height,
                                    round: input.vote.round,
                                    value: input.vote.id,
                                    vr: -1})    
    }
    else
        // none of the supported precommit events. Do nothing
        (es, defaultResult)  
}

// TODO: make sure only valid proposals are in es.proposals

// We do this if the executor receives a prevote
pure def Prevote (es: ExecutorState, input: ExecutorInput, bkr: BKResult) : (ExecutorState, Result) = {
    // TODO: events do not have heights now.
    // TODO: Polka implications missing.
    if (bkr.event.name == "PolkaValue")
        if (bkr.event.round < es.cs.round and
            es.proposals.exists(p => p.round == es.cs.round and
                bkr.event.round == p.validRound and p.proposal == bkr.event.value)) 
                callConsensus(es, bkr.bookkeeper,  {name : "ProposalAndPolkaPreviousAndValid",
                                    height : es.cs.height,
                                    round: es.cs.round,
                                    value: bkr.event.value,
                                    vr: bkr.event.round})   
                                    // TODO: the value should come from the proposal   
        else if (bkr.event.round == es.cs.round and
            es.proposals.exists(p => p.round == es.cs.round and
                p.proposal == bkr.event.value)) 
                callConsensus(es, bkr.bookkeeper,  {name : "ProposalAndPolkaAndValid",
                                    height : es.cs.height,
                                    round: es.cs.round,
                                    value: bkr.event.value,
                                    vr: bkr.event.round})   
        else
            // we don't have a matching proposal so we do nothing
            // TODO: we might check whether it is for a future round and jump
            (es, defaultResult)
    else if (bkr.event.name == "PolkaAny")
        if (bkr.event.round == es.cs.round)
                // call consensus and remember that we did it
                callConsensus(es, bkr.bookkeeper,  {name : "PolkaAny",
                                    height : es.cs.height,
                                    round: es.cs.round,
                                    value: bkr.event.value,
                                    vr: bkr.event.round})  
        else
            // TODO: we might check whether it is for a future round and jump
            (es, defaultResult)
    else if (bkr.event.name == "PolkaNil" and bkr.event.round == es.cs.round)
        callConsensus(es, bkr.bookkeeper,  {name : "PolkaNil",
                                    height : es.cs.height,
                                    round: es.cs.round,
                                    value: bkr.event.value,
                                    vr: bkr.event.round})  
    else
        (es, defaultResult)
}

// We do this if a timeout expires at the executor
pure def Timeout (es: ExecutorState, input: ExecutorInput) : (ExecutorState, Result) = {
    // TODO: We assume that the timeout event is always for the current round. If this is not
        // the case, we need to encode it in the input to which round the timeout belongs
    val event: Event = {name : input.timeout, height : es.cs.height,
                                    round: es.cs.round, value: "", vr: 0}
    callConsensus(es, es.bk, event)
}

// We do this if the executor receives a proposal
pure def ProposalMsg (es: ExecutorState, input: ExecutorInput) : (ExecutorState, Result) = {
    if (input.proposal.src != Proposer(es.cs.height, es.cs.round))
        (es, defaultResult)
    else if (not(valid(input.proposal))) {
        // do we n
        if (es.cs.step == "propose")
            if (checkThreshold(es.bk, es.cs.round, "Prevote", {name: "PolkaValue", value: input.proposal.proposal}))
                val event: Event = {name : "ProposalAndPolkaAndInValid", height : es.cs.height,
                                    round: es.cs.round, value: input.proposal.proposal, vr: 0}
                callConsensus(es, es.bk, event)
            else 
                val event: Event = {name : "ProposalInvalid", height : es.cs.height,
                                    round: es.cs.round, value: input.proposal.proposal, vr: 0}
                callConsensus(es, es.bk, event)
        else 
            (es, defaultResult)
    }
    else { // proposal valid and from proposer
        if (es.cs.step == "propose") 
            if (input.proposal.validRound == -1)
                if (checkThreshold(es.bk, es.cs.round, "Prevote", {name: "PolkaValue", value: input.proposal.proposal}))
                    callConsensus(es, es.bk,  {name : "ProposalAndPolkaAndValid",
                                    height : es.cs.height,
                                    round: es.cs.round,
                                    value: input.proposal.proposal,
                                    vr: input.proposal.round})   
                else
                    callConsensus(es, es.bk,  {name : "Proposal",
                                    height : es.cs.height,
                                    round: es.cs.round,
                                    value: input.proposal.proposal,
                                    vr: input.proposal.round})    
            else
                if (checkThreshold(es.bk, input.proposal.validRound, "Prevote", {name: "PolkaValue", value: input.proposal.proposal}))
                    callConsensus(es, es.bk,  {name : "ProposalAndPolkaPreviousAndValid",
                                    height : es.cs.height,
                                    round: es.cs.round,
                                    value: input.proposal.proposal,
                                    vr: input.proposal.round})  
                else  
                    (es, defaultResult)
        else if (es.cs.step == "prevote" or es.cs.step == "precommit") 
            if (checkThreshold(es.bk, es.cs.round, "Precommit", {name: "PrecommitValue", value: input.proposal.proposal}))
                callConsensus(es, es.bk,  {name : "ProposalAndCommitAndValid",
                                    height : es.cs.height,
                                    round: es.cs.round,
                                    value: input.proposal.proposal,
                                    vr: input.proposal.round})  
            // TODO: here we need to call both, Commit and Polka. We need to figure out iteratin
            else if (checkThreshold(es.bk, es.cs.round, "Prevote", {name: "PolkaValue", value: input.proposal.proposal}))
                callConsensus(es, es.bk,  {name : "ProposalAndPolkaAndValid",
                                    height : es.cs.height,
                                    round: es.cs.round,
                                    value: input.proposal.proposal,
                                    vr: input.proposal.round})  
            else 
                (es, defaultResult)
        else
            (es, defaultResult)
    }
}

// We do this when we need to jump to a new round
pure def skip (es: ExecutorState, r: int) : (ExecutorState, Result) = {
    val prop =  if (es.cs.validValue != "nil") es.cs.validValue
                else getValue(es.cs, r)
    if (Proposer(es.cs.height, es.cs.round + 1) == es.cs.p)
        callConsensus(es, es.bk,  {name : "NewRoundProposer",
                                    height : es.cs.height,
                                    round: r,
                                    // what value?
                                    value: prop,
                                    vr: es.cs.validRound})  
    else 
        callConsensus(es, es.bk,  {name : "NewRound",
                                    height : es.cs.height,
                                    round: r,
                                    value: "",
                                    vr: es.cs.validRound})  
    // what do we do now in the new round? Shouldn't we look whether we can build an event.
    // TODO: compute pending events.
}

// We do this when we have decided
pure def decided (es: ExecutorState, res: Result) : (ExecutorState, Result) = {
    // here we call consensus to set a new height, that is, to initialize the state machine
    // and then we call skip to start round 0
    // TODO: one can put res.decided into the blockstore
    val s1 = callConsensus(es, es.bk, {name : "NewHeight",
                                    height : es.cs.height + 1,
                                    round: -1,
                                    value: "",
                                    vr: es.cs.validRound})  
    skip (s1._1, 0)
}

// take input out of pending events and then call consensus with that event
// We do this when the executor is asked to work on pending events
pure def PendingEvent (es: ExecutorState, input: ExecutorInput) : (ExecutorState, Result) = {
    val newState = { ...es, pendingEvents: es.pendingEvents.exclude(Set((input.event, es.cs.height, es.cs.round)))}
    callConsensus(newState, es.bk, input.event)
}

pure def executor (es: ExecutorState, input: ExecutorInput) : (ExecutorState, Result) = {
    // TODO: shall we check whether the sender actually is in the validator set
    if (input.name == "proposal") {
        val cons_res = ProposalMsg(es, input)
        if (cons_res._2.name == "decide")
            decided (cons_res._1, cons_res._2)
        else
            cons_res
    }
    else if (input.name == "vote" and input.vote.step == "precommit") {
        val res = applyVote(es.bk, toVote(input.vote), es.valset.get(input.vote.src))
        // only a commit event can come here. 
        val cons_res = Precommit(es, input, res) 
        if (cons_res._2.name == "decide")
            decided (cons_res._1, cons_res._2)
        else
            cons_res
    }
    else if (input.name == "vote" and input.vote.step == "prevote") {
        val res = applyVote(es.bk, toVote(input.vote), es.valset.get(input.vote.src))
        // only a commit event can come here. 
        Prevote(es, input, res) 
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
        val vote = state.incomingVotes.fold(emptyVote,  (sum, y) => y)
        val newstate = { ...state, incomingVotes: state.incomingVotes.exclude(Set(vote))}
        (newstate, { ...defaultInput, name: "vote", vote: vote})
    else if (state.timeout.exists(x => x._2 == state.es.cs.height and 
        x._3 == state.es.cs.round))
        // pick timeout, remove it from incoming
        val timeouts = state.timeout.filter(x => x._2 == state.es.cs.height and 
            x._3 == state.es.cs.round)
        // val timeout = timeouts.chooseSome()
        val timeout = timeouts.fold(("", 0, 0),   (sum, y) => y)
        val newstate = { ...state, timeout: state.timeout.exclude(Set(timeout))}
        (newstate, { ...defaultInput, name: "timeout", timeout: timeout._1})
    else if (state.es.pendingEvents != Set())
        // this might be cheating as we look into the "es"
        (state, { ...defaultInput, name: "pending" })
    else
        (state, defaultInput)
}

}