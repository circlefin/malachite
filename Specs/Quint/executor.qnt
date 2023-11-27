// -*- mode: Bluespec; -*-

/*
  TODO: round switch upon f+1 messages from the future is not done yet. We need to catch
  the event from the bookkeeper
*/ 

module executor {

import consensus.* from "./consensus"
import voteBookkeeper.* from "./voteBookkeeper"

pure def initBookKeeper(totalVotingPower: int): Bookkeeper = 
    { height: 0, totalWeight: totalVotingPower, rounds: Map() }


type ExecutorState = {
    bk : Bookkeeper,
    cs : ConsensusState,
    proposals: Set[ProposeMsg_t],
    valset: Address_t -> int,
    executedEvents: List[(Event, Height_t, Round_t)], // We record that to have the information in the trace
    pendingEvents: Set[(Event, Height_t, Round_t)],
    started: bool,
    applyvotesResult: ExecutorEvent, //debug TODO
    chain : List[Value_t],
    nextValueToPropose: Value_t,
}

pure def initExecutor  (v: Address_t, vs: Address_t -> int) : ExecutorState = {
    val tvp = vs.keys().fold(0, (sum, key) => sum + vs.get(key))
    {
    bk: initBookKeeper(tvp),
    cs: initConsensusState(v),
    proposals: Set(),
    valset: vs,
    executedEvents: List(),
    pendingEvents: Set(),
    started: false,
    applyvotesResult: toEvent(0, "", {name: "", value: ""}), // debug
    chain : List(),
    nextValueToPropose: "", 
    }
}

type NodeState = {
    es: ExecutorState,
    timeout: Set[(Timeout_t, Height_t, Round_t)],
    incomingVotes: Set[VoteMsg_t],
    incomingProposals: Set[ProposeMsg_t],
}

pure def initNode  (v: Address_t, vs: Address_t -> int) : NodeState = {
    es: initExecutor(v,vs),
    timeout: Set(),
    incomingVotes: Set(),
    incomingProposals: Set(),
}

type ExecutorInput = {
    name: str, // TODO: make a set of values. 
    proposal: ProposeMsg_t,
    vote: VoteMsg_t,
    timeout: str,
    event: Event,
    nextValueToPropose: Value_t
}

val defaultInput: ExecutorInput = {
    name: "",
    proposal: { src: "", height: 0, round: 0, proposal: "", validRound: 0 },
    vote: { src: "", height: 0, round: 0, step: "", id: "", },
    timeout: "",
    event: defaultEvent,
    nextValueToPropose: ""
}

// this is to match the type from the bookkeeper. When we add heights there we should
// unify
pure def toVote (v: VoteMsg_t) : Vote = 
    { typ: v.step, height: v.height, round: v.round, value: v.id, address: v.src }

val defaultEvent : Event = { name : "", height : 0, round: 0, value: "", vr: 0 }    

/*
encode the following decision tree

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

// In the implementation this could be a callback to the application. But it needs to be
// a function, that is, any two validators need to agree on this
pure def Proposer(valset: Address_t -> int, height: Height_t, round: Round_t) : Address_t = {
    // Here: rotating coordinator. We can do something more clever actually using the valset
    val prop = (round + 1) % 4
    if (prop == 0) "v1"
    else if  (prop == 1) "v2"
    else if  (prop == 2) "v3"
    else "v4"
}

pure def getValue(es: ExecutorState) : Value_t =  es.nextValueToPropose

pure def valid(p: ProposeMsg_t) :bool = {
    // for simulation, if the value is "invalid", so is the proposal
    // if this is to become "non-deterministic", we must push it
    // and its use into the state machine
    p.proposal != "invalid"
}

// efficient hashing function
pure def id(v) = v 


type ConsensusCall = {
    es: ExecutorState,
    event: Event,
    out: ConsensusOutput
}

pure def ListContains(list, value) = 
    list.foldl(false, (s,x) => s or x == value)

// check whether the event has been already sent to consensus. If not, do so.
pure def callConsensus (es: ExecutorState, bk: Bookkeeper, ev: Event) : (ExecutorState, ConsensusOutput) = {
    // Check whether we already executed the event already
    if (es.executedEvents.ListContains((ev, es.cs.height, es.cs.round)))
        ({ ...es, bk: bk, cs: es.cs }, defaultResult)
    else
        // Go to consensus
        val res = consensus(es.cs, ev)
        // Record that we executed the event
        val events = es.executedEvents.append((ev, res.cs.height, res.cs.round))

        ({ ...es, bk: bk, cs: res.cs, executedEvents: events }, res.out)
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
    else if (eev.name == "Skip" and eev.round > es.cs.round)
        callConsensus(es, es.bk,  { name: "RoundSkip",
                                    height: input.vote.height,
                                    round: input.vote.round,
                                    value: input.vote.id,
                                    vr: -1}) 
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
    else if (eev.name == "Skip" and eev.round > es.cs.round)
        callConsensus(es, es.bk,  { name: "RoundSkip",
                                    height: input.vote.height,
                                    round: input.vote.round,
                                    value: input.vote.id,
                                    vr: -1}) 
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
    if (input.proposal.src != Proposer(es.valset, input.proposal.height, input.proposal.round))
        // proposer does not match the height/round of the proposal
        // keep ES (don't use newES here), that is, drop proposal
        (es, defaultResult)
    else if (valid(input.proposal)) 
        val receivedCommit = checkThreshold(    newES.bk, 
                                                input.proposal.round, 
                                                "Precommit", 
                                                {name: "Value", 
                                                value: id(input.proposal.proposal)})
        if (receivedCommit)
            // we have a commit that matches the proposal. We don't need to compare against
            // es.cs.round
            // TODO: check heights in bookkeeper
            callConsensus(  newES, 
                            newES.bk,  
                                {   name: "ProposalAndCommitAndValid",
                                    height: input.proposal.height,
                                    round: input.proposal.round,
                                    value: id(input.proposal.proposal),
                                    vr: input.proposal.validRound})  
        else if (input.proposal.round != es.cs.round or input.proposal.height != es.cs.height) 
            // the proposal is from the right proposer and valid, but not for this round
            // keep the proposal, do nothing else
            (newES, defaultResult)
        else
            // for current round and q, valid, and from right proposer
            val receivedPolkaValidRoundVal = checkThreshold(newES.bk,   
                                                            input.proposal.validRound, 
                                                            "Prevote", 
                                                            {name: "Value", 
                                                            value: id(input.proposal.proposal)})
            val receivedPolkaCurrentVal = checkThreshold(   newES.bk, 
                                                            newES.cs.round, 
                                                            "Prevote", 
                                                            {name: "Value", 
                                                            value: id(input.proposal.proposal)})
            val receivedCommitCurrentVal = checkThreshold(  newES.bk, 
                                                            newES.cs.round, 
                                                            "Precommit", 
                                                            {name: "Value", 
                                                            value: id(input.proposal.proposal)})
            if (newES.cs.step == "propose") 
                if (input.proposal.validRound == -1)
                    if (receivedPolkaCurrentVal)
                        callConsensus(  newES, 
                                        newES.bk,  
                                        {   name: "ProposalAndPolkaAndValid",
                                            height: input.proposal.height,
                                            round: input.proposal.round,
                                            value: id(input.proposal.proposal),
                                            vr: input.proposal.validRound})   
                    else
                        callConsensus(  newES, 
                                        newES.bk,  
                                        {   name: "Proposal",
                                            height: input.proposal.height,
                                            round: input.proposal.round,
                                            value: id(input.proposal.proposal),
                                            vr: input.proposal.validRound})    
                else if (receivedPolkaValidRoundVal)
                    callConsensus(  newES, 
                                    newES.bk,  
                                    {   name: "ProposalAndPolkaPreviousAndValid",
                                        height: input.proposal.height,
                                        round: input.proposal.round,
                                        value: id(input.proposal.proposal),
                                        vr: input.proposal.validRound})  
                else  
                    (newES, defaultResult)
            else if (newES.cs.step == "Prevote" or newES.cs.step == "Precommit") 
                if (receivedCommitCurrentVal)
                    // here we need to call both, Commit and Polka. 
                    // We do commit and append pola to pending
                    val pend = ( {  name: "ProposalAndPolkaAndValid",
                                    height: newES.cs.height,
                                    round: newES.cs.round,
                                    value: id(input.proposal.proposal),
                                    vr: input.proposal.validRound},
                                newES.cs.height,
                                newES.cs.round)
                    callConsensus(  { ...newES, pendingEvents: newES.pendingEvents.union(Set(pend))}, 
                                    newES.bk,  
                                    {   name: "ProposalAndCommitAndValid",
                                        height: newES.cs.height,
                                        round: newES.cs.round,
                                        value: id(input.proposal.proposal),
                                        vr: input.proposal.validRound})  
                else if (receivedPolkaCurrentVal)
                    callConsensus(  newES , 
                                    newES.bk, 
                                    {   name: "ProposalAndPolkaAndValid",
                                    height: newES.cs.height,
                                    round: newES.cs.round,
                                    value: id(input.proposal.proposal),
                                    vr: input.proposal.validRound})  
                else 
                    (newES, defaultResult)
            else
                (newES, defaultResult)
    else 
        // (not(valid(input.proposal))) 
        // keep ES (don't use newES here), that is, drop proposal
        if (es.cs.step == "propose" and es.cs.round == input.proposal.round and es.cs.height == input.proposal.height)
            if (checkThreshold(es.bk, es.cs.round, "Prevote", {name: "Value", value: id(input.proposal.proposal)}))
                val event: Event = {name: "ProposalAndPolkaAndInValid", 
                                    height: es.cs.height,
                                    round: es.cs.round, 
                                    value: id(input.proposal.proposal), 
                                    vr: input.proposal.validRound}
                callConsensus(es, es.bk, event)
            else 
                val event: Event = {name: "ProposalInvalid", 
                                    height: es.cs.height,
                                    round: es.cs.round, 
                                    value: id(input.proposal.proposal), 
                                    vr: input.proposal.validRound}
                callConsensus(es, es.bk, event)
        else 
            (es, defaultResult)
}


// We do this when we need to jump to a new round
pure def skip (es: ExecutorState, r: int) : (ExecutorState, ConsensusOutput) = {
    // line 15
    val prop =  if (es.cs.validValue != "nil") es.cs.validValue
                else getValue(es)
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

/*  // The following can be used to get to the next height. For now this
    // function does nothing
    // If we choose to move getValue out of the executor logic into the environment (gossip)
    // then, we would not do this here, but expect the environment to create a (to be defined) 
    // ExecutorInput      
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


pure def setValue(es: ExecutorState, value: Value_t) : (ExecutorState, ConsensusOutput) =
    ({ ...es, nextValueToPropose: value }, defaultResult)




/* *********************************************************
 * Main entry point
 * ********************************************************/

// TODO: return ConsensusEvent so that we know from outside what event was fired.
pure def executor (es: ExecutorState, input: ExecutorInput) : (ExecutorState, ConsensusOutput) = {
    // TODO: shall we check whether the sender actually is in the validator set
    if (input.name == "proposal") {
        val res = ProposalMsg(es, input)
        if (res._2.name == "decided")
            decided (res._1, res._2)
        else if (res._2.name == "skipRound")
            skip (res._1, res._2.skipRound)
        else
            res
    }
    else if (input.name == "votemessage" and input.vote.step == "Precommit") {
        val res = applyVote(es.bk, toVote(input.vote), es.valset.get(input.vote.src), es.cs.round)
        val newES = { ...es, bk: res.bookkeeper, applyvotesResult: res.event}
        // only a commit event can come here. 
        val cons_res = Precommit(newES, input, res.event) 
        if (cons_res._2.name == "decided")
            decided (cons_res._1, cons_res._2)
        else if (cons_res._2.name == "skipRound")
            skip (cons_res._1, cons_res._2.skipRound)
        else
            cons_res
    }
    else if (input.name == "votemessage" and input.vote.step == "Prevote") {
        val res = applyVote(es.bk, toVote(input.vote), es.valset.get(input.vote.src), es.cs.round)
        val newES = { ...es, bk: res.bookkeeper, applyvotesResult: res.event}
        // only a commit event can come here. 
        val cons_res = Prevote(newES, input, res.event) 
        if (cons_res._2.name == "decided")
            // TODO: dead branch. But we should put this after consensus call logic into a function
            decided (cons_res._1, cons_res._2)
        else if (cons_res._2.name == "skipRound")
            skip (cons_res._1, cons_res._2.skipRound)
        else
            cons_res
    }
    else if (input.name == "timeout") {
        val res = Timeout(es, input)
        // result should be vote or skip
        if (res._2.name == "skipRound")
            skip (res._1, res._2.skipRound)
            // skip starts a new round. This may involve getValue. If we choose to move the getValue
            // logic out of the executor, we wouldn't call skip here but add a (to be defined)
            // ExecutorInput
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
    else if (input.name == "SetNextProposedValue")
        setValue(es, input.nextValueToPropose)
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
        // TODO: In the "starkBFT Spec" Google doc, it is written that pending events
        // should be executed before new messages, which would requir to push this
        // branch up.
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
