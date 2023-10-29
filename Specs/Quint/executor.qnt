// -*- mode: Bluespec; -*-

module executor {

import consensus.* from "./consensus"
import voteBookkeeper.* from "./voteBookkeeper"


type ExecutorState = {
    bk : Bookkeeper,
    cs : ConsensusState,
    proposals: Set[ProposeMsg_t],
    valset: Address_t -> int,
    timeoutPrevote: Set[(Height_t, Round_t)],
    timeoutPrecommit: Set[(Height_t, Round_t)]
}

type NodeState = {
    es: ExecutorState,
    incomingVotes: Set[VoteMsg_t],
    incomingProposals: Set[ProposeMsg_t],
}

type ExecutorInput = {
    name: str,
    proposal: ProposeMsg_t,
    vote: VoteMsg_t,
    timeout: str
}

val defaultInput = {name: "",
    proposal: {
    src: "",
    height: 0,
    round: 0,
    proposal: "",
    validRound: 0
    },
    vote: {
    src: "",
    height: 0,
    round: 0,
    step: "",  // "prevote" or "precommit"
    id: "",
    },
    timeout: ""}


//this is a hack. We need to understand duplicate votes eventually
pure def toVote (v: VoteMsg_t) : Vote = {
    typ: v.step, 
    round: v.round,
    value: v.id,
    address: v.src
}

val defaultEvent = {
    name : "",
    height : 0,
    round: 0,
    value: "",
    vr: 0
}    

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



val emptyProposal : ProposeMsg_t= {
    src: "none",
    height: 0,
    round: 0,
    proposal: "none",
    validRound: 0
}

pure def Proposer(height: Height_t, round: Round_t) : Address_t = "The Proposer"
pure def valid (v) = true
pure def id(v) = v // efficient hashing function
pure def getProposal(cs: ConsensusState, round:int) : Value_t = "empty"


val emptyVote = {
    src: "none",
    height: 0,
    round: 0,
    step: "None",  // "prevote" or "precommit"
    id: "None",
  }


pure def callConsensus (es: ExecutorState,  bk: Bookkeeper, ev: Event) : (ExecutorState, Result) = {
    val res = consensus(es.cs, ev)
     ({ ...es, bk: bk, cs: res._1}, res._2)
}

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
    else if (bkr.event.name == "PrecommitAny" and bkr.event.round == es.cs.round and
            not(es.timeoutPrecommit.exists(x => x._2 == bkr.event.round))) {
                val res = callConsensus(es, bkr.bookkeeper,  {name : "PrecommitAny",
                                    height : input.vote.height,
                                    round: input.vote.round,
                                    value: input.vote.id,
                                    vr: -1})    
                // call consensus and remember that we did it                  
                ({ ...res._1, timeoutPrecommit: res._1.timeoutPrecommit.union(Set((es.cs.height, es.cs.round)))}, res._2) 
    }
    else
        // none of the supported precommit events. Do nothing
        (es, defaultResult)  
}
// TODO: make sure only valid proposals are in es.proposals

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
        if (bkr.event.round == es.cs.round and 
            not(es.timeoutPrevote.exists(x => x._2 == bkr.event.round)))
                // call consensus and remember that we did it
                val res = callConsensus(es, bkr.bookkeeper,  {name : "PolkaAny",
                                    height : es.cs.height,
                                    round: es.cs.round,
                                    value: bkr.event.value,
                                    vr: bkr.event.round})  
                ({ ...res._1, timeoutPrevote: res._1.timeoutPrevote.union(Set((es.cs.height, es.cs.round)))}, res._2) 
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

pure def Timeout (es: ExecutorState, input: ExecutorInput) : (ExecutorState, Result) = {
    // TODO: We assume that the timeout event is always for the current round. If this is not
        // the case, we need to encode it in the input to which round the timeout belongs
    val event: Event = {name : input.timeout, height : es.cs.height,
                                    round: es.cs.round, value: "", vr: 0}
    callConsensus(es, es.bk, event)
}




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

 //   "decided",
 //   "skipRound"

pure def skip (es: ExecutorState, r: int) : (ExecutorState, Result) = {
    val prop =  if (es.cs.validValue != "nil") getProposal(es.cs, r)
                else es.cs.validValue
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
}

pure def decided (es: ExecutorState, res: Result) : (ExecutorState, Result) = {
    // here we call consensus to set a new height, that is, to initialize the state machine
    // and then we call skip to start round 0
    // TODO: one can put res.decided into the blockstore
    val s1 = callConsensus(es, es.bk, {name : "NewHeight",
                                    height : es.cs.height + 1,
                                    round: 0,
                                    value: "",
                                    vr: es.cs.validRound})  
    skip (s1._1, 0)
}


pure def executor (es: ExecutorState, input: ExecutorInput) : (ExecutorState, Result) = {
    // TODO: shall we check whether the sender actually is in the validator set
    if (input.name == "proposal") {
        ProposalMsg(es, input)
    }
    else if (input.name == "vote" and input.vote.step == "precommit") {
        val res = applyVote(es.bk, toVote(input.vote), es.valset.get(input.vote.src))
        // only a commit event can come here. 
        Precommit(es, input, res) 
    }
    else if (input.name == "vote" and input.vote.step == "prevote") {
        val res = applyVote(es.bk, toVote(input.vote), es.valset.get(input.vote.src))
        // only a commit event can come here. 
        val cons_res = Prevote(es, input, res) 
        if (cons_res._2.name == "decide")
            decided (cons_res._1, cons_res._2)
        else
            cons_res
    }
    else if (input.name == "timeout") {
        val res = Timeout(es, input)
        // result should be vote or skip
        if (res._2.name == "skipRound")
            skip (res._1, res._2.skipRound)
        else
            res
    }
    else
        (es, defaultResult)
}




val validators = Set("v1", "v2", "v3", "v4")
val valset = validators.mapBy(x => 1)

var system : Address_t -> NodeState

//var _Result : Result
//var _Event : Event

action actTimeoutPrevote (validator: Address_t) : bool = all {
    // check that the timeout is started
    system.get(validator).es.timeoutPrevote.exists(x => 
                x._1 == system.get(validator).es.cs.height and
                x._2 == system.get(validator).es.cs.round),
    val res = executor(system.get(validator).es, { ... defaultInput, name: "timeout", timeout: "TimeoutPrevote"})
    //TODO: deliver proposals
    //val newsys = system.keys().mapBy(x => 
    //    { ...system.get(x), incomingProposals : system.get(x).incomingProposals.union(Set(emptyProposal)) } )
    val v = { ...system.get(validator), es: res._1}
    // TODO: stop the timeout?
    system' = system.set(validator, v), 
}

action step = {
    nondet v = oneOf(validators)
    any {
        actTimeoutPrevote(v),
    }

}


}