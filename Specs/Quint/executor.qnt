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

type ExecutorInput = {
    name: str,
    proposal: ProposeMsg_t,
    vote: VoteMsg_t,
    timeout: str
}

type ExecutorOutput = {
    name: str,
    proposal: ProposeMsg_t,
    vote: VoteMsg_t, 
    timeout: str  
}

//this is a hack. We need to understand duplicate votes eventually
pure def toVote (v: VoteMsg_t) : Vote = {
    typ: v.step, 
    round: v.round,
    value: v.id,
    address: v.src
}

pure def id(v) = v // efficient hashing function
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

pure def Proposer(height: Height_t, round: Round_t) : Address_t = "Josef"

pure def computeOutput(res: Result) : ExecutorOutput = {
    name: "TODO",
    proposal: {
    src: "Josef",
    height: 0,
    round: 0,
    proposal: "Manu",
    validRound: 0
  },
    vote: {
    src: "Josef",
    height: 0,
    round: 0,
    step: "None",  // "prevote" or "precommit"
    id: "None",
  }, 
    timeout: "None"  
}

pure def callConsensus (es: ExecutorState,  bkr: BKResult, ev: Event) : (ExecutorState, Set[ExecutorOutput]) = {
    val res = consensus(es.cs, ev)
    ({ ...es, bk: bkr.bookkeeper, cs: res._1}, Set(computeOutput(res._2)))
}

pure def Precommit (es: ExecutorState, input: ExecutorInput, bkr: BKResult) : (ExecutorState, Set[ExecutorOutput]) = {
    if (bkr.event.name == "PrecommitValue")
            if (es.proposals.exists(x => bkr.event.round == x.round and bkr.event.value == x.proposal)) {
                callConsensus(es, bkr, {name : "ProposalAndCommitAndValid",
                                    height : input.vote.height,
                                    round: input.vote.round,
                                    value: input.vote.id,
                                    vr: -1})
            }
            else {
                if (bkr.event.round == es.cs.round) {
                    callConsensus(es, bkr,  {name : "PrecommitAny",
                                    height : input.vote.height,
                                    round: input.vote.round,
                                    value: input.vote.id,
                                    vr: -1})   
                }
                else if (bkr.event.round > es.cs.round) {
                    // if it is for a future round I can trigger skipround
                    // TODO: should we really do this. It is dead code as the f+1 event already happened
                    callConsensus(es, bkr,  {name : "RoundSkip",
                                    height : input.vote.height,
                                    round: input.vote.round,
                                    value: input.vote.id,
                                    vr: -1})     
                }  
                else {
                    // messages from past round -> ignore
                    (es, Set(input))
                }    
            }
    else if (bkr.event.name == "PrecommitAny" and bkr.event.round == es.cs.round and
            not(es.timeoutPrecommit.exists(x => x._2 == bkr.event.round))) {
                val res = callConsensus(es, bkr,  {name : "PrecommitAny",
                                    height : input.vote.height,
                                    round: input.vote.round,
                                    value: input.vote.id,
                                    vr: -1})    
                // call consensus and remember that we did it                  
                ({ ...res._1, timeoutPrecommit: res._1.timeoutPrecommit.union(Set((es.cs.height, es.cs.round)))}, res._2) 
    }
    else
        // none of the supported precommit events. Do nothing
        (es, Set(input))  
}
// TODO: make sure only valid proposals are in es.proposals

pure def Prevote (es: ExecutorState, input: ExecutorInput, bkr: BKResult) : (ExecutorState, Set[ExecutorOutput]) = {
    // TODO: events do not have heights now.
    if (bkr.event.name == "PolkaValue")
        if (bkr.event.round < es.cs.round and
            es.proposals.exists(p => p.round == es.cs.round and
                bkr.event.round == p.validRound and p.proposal == bkr.event.value)) 
                callConsensus(es, bkr,  {name : "ProposalAndPolkaPreviousAndValid",
                                    height : es.cs.height,
                                    round: es.cs.round,
                                    value: bkr.event.value,
                                    vr: bkr.event.round})   
                                    // TODO: the value should come from the proposal   
        else if (bkr.event.round == es.cs.round and
            es.proposals.exists(p => p.round == es.cs.round and
                p.proposal == bkr.event.value)) 
                callConsensus(es, bkr,  {name : "ProposalAndPolkaAndValid",
                                    height : es.cs.height,
                                    round: es.cs.round,
                                    value: bkr.event.value,
                                    vr: bkr.event.round})   
        else
            // we don't have a matching proposal so we do nothing
            // TODO: we might check whether it is for a future round and jump
            (es, Set(input))
    else if (bkr.event.name == "PolkaAny")
        if (bkr.event.round == es.cs.round and 
            not(es.timeoutPrevote.exists(x => x._2 == bkr.event.round)))
                // call consensus and remember that we did it
                val res = callConsensus(es, bkr,  {name : "PolkaAny",
                                    height : es.cs.height,
                                    round: es.cs.round,
                                    value: bkr.event.value,
                                    vr: bkr.event.round})  
                ({ ...res._1, timeoutPrevote: res._1.timeoutPrevote.union(Set((es.cs.height, es.cs.round)))}, res._2) 
        else
            // TODO: we might check whether it is for a future round and jump
            (es, Set(input))
    else if (bkr.event.name == "PolkaNil" and bkr.event.round == es.cs.round)
        callConsensus(es, bkr,  {name : "PolkaNil",
                                    height : es.cs.height,
                                    round: es.cs.round,
                                    value: bkr.event.value,
                                    vr: bkr.event.round})  
    else
        (es, Set(input))
}

pure def executor (es: ExecutorState, input: ExecutorInput) : (ExecutorState, Set[ExecutorOutput]) = {
    // TODO: shall we check whether the sender actually is in the validator set
    if (input.name == "proposal" and input.proposal.src == Proposer(es.cs.height, es.cs.round)) {
        // code after iterator is done
        ({ ...es, proposals: es.proposals.union(Set(input.proposal))}, Set(input))
    }
    else if (input.name == "vote" and input.vote.step == "precommit") {
        val res = applyVote(es.bk, toVote(input.vote), es.valset.get(input.vote.src))
        // only a commit event can come here. 
        Precommit(es, input, res) 
    }
    else if (input.name == "vote" and input.vote.step == "prevote") {
        val res = applyVote(es.bk, toVote(input.vote), es.valset.get(input.vote.src))
        // only a commit event can come here. 
        Prevote(es, input, res) 
    }
    else if (input.name == "timeout") {
        // TODO: We assume that the timeout event is always for the current round. If this is not
        // the case, we need to encode it in the input to which round the timeout belongs
        val res = consensus(es.cs, {name : input.timeout, height : es.cs.height,
                                    round: es.cs.round, value: "", vr: 0})
        ({ ...es, cs: res._1}, Set(computeOutput(res._2)))
    }
    else
        (es, Set(input))
}

// Old
// pure def executor (es: ExecutorState, input: ExecutorInput) : (ExecutorState, ExecutorOutput) = {
//     if (input.name == "vote")
//         val res = applyVote(es.bk, toVote(input.vote), es.valset.get(input.vote.src))
//         val consInput = executorLogic(res, es)
//         val consResult = consensus(es.cs, consInput)
//         (es, input)
//     else
//     (es, input)
// }




val validators = Set("v1", "v2", "v3", "v4")
val valset = validators.mapBy(x => 1)

var system : Address_t -> ConsensusState
var _Result : Result
var _Event : Event





}