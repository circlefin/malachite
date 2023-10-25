// -*- mode: Bluespec; -*-

module executor {

import consensus.* from "./consensus"
import voteBookkeeper.* from "./voteBookkeeper"


type ExecutorState = {
    bk : Bookkeeper,
    cs : ConsensusState,
    proposal: ProposeMsg_t,
    valset: Address_t -> int
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
    value: v.id
}

pure def id(v) = v // efficient hashing function

pure def executorLogic (res: BKResult, es: ExecutorState) : Event = {
    if (res.event.name == "PolkaValue")
        if (res.event.value == id(es.proposal.proposal)) {
         //   val event = if 
          //  ProposalAndPolkaAndValid
            {name: "ProposalAndPolkaPreviousAndValid",
             height: es.cs.height,
             round: es.cs.round,
             value: es.proposal.proposal,
             vr: -1 }
        }
        else defaultEvent
    else
    defaultEvent
}

pure def executor (es: ExecutorState, input: ExecutorInput) : (ExecutorState, ExecutorOutput) = {
    if (input.name == "vote")
        val res = applyVote(es.bk, toVote(input.vote), es.valset.get(input.vote.src))
        val consInput = executorLogic(res, es)
        val consResult = consensus(es.cs, consInput)
        (es, input)
    else
    (es, input)
}




val validators = Set("v1", "v2", "v3", "v4")
val valset = validators.mapBy(x => 1)

var system : Address_t -> ConsensusState
var _Result : Result
var _Event : Event





}