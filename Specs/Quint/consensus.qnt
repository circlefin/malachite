// -*- mode: Bluespec; -*-

module consensus {

  // a process name is just a string in our specification
  type Address_t = str
  // a value is also a string
  type Value_t = str
  // a state is also a string
  type Step_t = str
  // a round is an integer
  type Round_t = int
  // a height is an integer
  type Height_t = int
  // a state is also a string
  type Timeout_t = str

// the type of propose messages
  type ProposeMsg_t = {
    src: Address_t,
    height: Height_t,
    round: Round_t,
    proposal: Value_t,
    validRound: Round_t
  }

  // the type of Prevote and Precommit messages
  type VoteMsg_t = {
    src: Address_t,
    height: Height_t,
    round: Round_t,
    step: Step_t,  // "prevote" or "precommit"
    id: Value_t,
  }

type ConsensusState = {
    p: Address_t,
    height : int,
    round: Round_t,
    step: Step_t, // "newRound", propose, prevote, precommit, decided
    lockedRound: Round_t,
    lockedValue: Value_t,
    validRound: Round_t,
    validValue: Value_t,
    //continue
}  

type Event = {
    name : str,
    height : int,
    round: Round_t,
    value: Value_t,
    vr: Round_t
}    

// what is a good way to encode optionals? I do with default values
type Result = {
    name : str,    
    proposal: ProposeMsg_t,
    voteMessage: VoteMsg_t,
    timeout: Timeout_t,
    decided: Value_t,
    skipRound: Round_t
}

val consensusEvents = Set(
    "NewHeight",
    "NewRound",                // Start a new round, not as proposer.
    "NewRoundProposer(Value)", // Start a new round and propose the Value.
    "Proposal",      // Receive a proposal with possible polka round.
    "ProposalAndPolkaPreviousAndValid", //28 when valid
    "ProposalInvalid", // 26 and 32 when invalid in step propose
    "PolkaNil",                // Receive +2/3 prevotes for nil.
    "PolkaAny",                // Receive +2/3 prevotes for anything and they are not the same
    "ProposalAndPolkaAndValid", // 36 when valid and step >= prevote
    "PrecommitAny",           // Receive +2/3 precommits for anything.
    "ProposalAndCommitAndValid", // decide
    "RoundSkip",               // Receive +1/3 votes from a higher round.
    "TimeoutPropose",          // Timeout waiting for proposal.
    "TimeoutPrevote",          // Timeout waiting for prevotes.
    "TimeoutPrecommit"       // Timeout waiting for precommits
)

/*    

    "PolkaValue(ValueId)",     // Receive +2/3 prevotes for Value.
    "PrecommitValue(ValueId)", // Receive +2/3 precommits for Value.

) */

val noProp : ProposeMsg_t = {src: "", height: -1, round: -1, proposal: "", validRound: -1}
val noVote : VoteMsg_t = {src: "", height: -1, round: -1, step: "", id: ""}
val noTimeout : Timeout_t = ""
val noDecided = ""
val noSkipRound : Round_t = -1
val defaultResult : Result = { 
                name: "",
                proposal: noProp, 
                voteMessage: noVote, 
                timeout: noTimeout, 
                decided: noDecided,
                skipRound: noSkipRound}


pure def NewHeight (state: ConsensusState, ev: Event) : (ConsensusState, Result) = {
    val newstate = { ...state, round: ev.round, 
                            step: "newRound", 
                            height : ev.height,
                            lockedRound: -1, 
                            lockedValue: "nil", 
                            validRound: -1, 
                            validValue: "nil"
                            }
    (newstate, defaultResult)
}

// line 11.14
pure def NewRoundProposer (state: ConsensusState, ev: Event) : (ConsensusState, Result) = {
    val newstate = { ...state, round: ev.round, step: "propose"}
    val proposal =  if (state.validValue != "nil") state.validValue
                    else ev.value
    val result = { ...defaultResult, name: "proposal",
                                     proposal: {   src: state.p, 
                                                        height: state.height,
                                                        round: ev.round, 
                                                        proposal: proposal,
                                                        validRound: state.validRound}}
    (newstate, result)
}

// line 11.20
pure def NewRound (state: ConsensusState, ev: Event) : (ConsensusState, Result) = {
    val newstate = { ...state, round: ev.round, step: "propose" }
    val result = { ...defaultResult, name: "timeout", timeout: "timeoutPropose"} // do we need the roundnumber here?
        (newstate, result)
}

// line 22
pure def Proposal (state: ConsensusState, ev: Event) : (ConsensusState, Result) = {
    if (state.step == "propose") {
        val newstate = state.with("step", "prevote")
        if (state.lockedRound == -1 or state.lockedValue == ev.value)
            val result = { ...defaultResult, name: "votemessage",
                                             voteMessage: { src: state.p,
                                                            height: state.height,            
                                                            round: state.round,
                                                            step: "prevote",  // "prevote" or "precommit"
                                                            id: ev.value}}
            (newstate, result)
        else
            val result = { ...defaultResult, name: "votemessage",
                                             voteMessage: { src: state.p,
                                                            height: state.height,            
                                                            round: state.round,
                                                            step: "prevote",  // "prevote" or "precommit"
                                                            id: "nil"}}
            (newstate, result)
    }
    else
        (state, defaultResult)
}

// line 28
pure def ProposalAndPolkaPreviousAndValid (state: ConsensusState, ev: Event) : (ConsensusState, Result) = {
    if (state.step == "propose" and ev.vr >= 0 and ev.vr < state.round) {
        val newstate = state.with("step", "prevote")
        if (state.lockedRound <= ev.vr or state.lockedValue == ev.value)
            val result = { ...defaultResult, name: "votemessage",
                                             voteMessage: { src: state.p,
                                                            height: state.height,
                                                            round: state.round,
                                                            step: "prevote",  // "prevote" or "precommit"
                                                            id: ev.value}}
            (newstate, result)
        else
            val result = { ...defaultResult, name: "votemessage",
                                             voteMessage: { src: state.p,
                                                            height: state.height,
                                                            round: state.round,
                                                            step: "prevote",  // "prevote" or "precommit"
                                                            id: "nil"}}
            (newstate, result)                                             
    }
    else
        (state, defaultResult)
}

pure def ProposalInvalid (state: ConsensusState, ev: Event) : (ConsensusState, Result) = {
    if (state.step == "propose") {
        val newstate = state.with("step", "prevote")
        val result = { ...defaultResult, name: "votemessage",
                                         voteMessage: { src: state.p,
                                                            height: state.height,
                                                            round: state.round,
                                                            step: "prevote",  // "prevote" or "precommit"
                                                            id: "nil"}}
        (newstate, result)
    }
    else {
        (state, defaultResult )      
    }
}

// line 34
pure def PolkaAny (state: ConsensusState, ev: Event) : (ConsensusState, Result) = {
    if (state.step == "prevote") {
        val result = { ...defaultResult, name: "timeout", timeout: "timeoutPrevote" } // do we need the roundnumber here?
        (state, result)
    }
    else
        (state, defaultResult) 
}

// line 36
pure def ProposalAndPolkaAndValid (state: ConsensusState, ev: Event) : (ConsensusState, Result) = {
    val auxState = { ...state, validValue: ev.value, validRound: state.round }
    if (state.step == "prevote") {
        val newstate = { ...auxState,   lockedValue: ev.value,
                                        lockedRound: state.round,
                                        step: "precommit" }
        val result = { ...defaultResult, name: "votemessage",
                                         voteMessage: { src: state.p,
                                                        height: state.height,
                                                        round: state.round,
                                                        step: "precommit",  
                                                        id: ev.value}}
        (newstate, result)
        }
    else {
        (state, defaultResult)
    }
}

// line 44
pure def PolkaNil (state: ConsensusState, ev: Event) : (ConsensusState, Result) = {
    if (state.step != "prevote")
        (state, defaultResult)
    else
        val newstate = { ...state, step: "precommit"}
        val result = { ...defaultResult, name: "votemessage",
                                         voteMessage: { src: state.p,
                                                        height: state.height,
                                                        round: state.round,
                                                        step: "precommit",  
                                                        id: "nil"}}
        (newstate, result)
}

// line 47
pure def PrecommitAny (state: ConsensusState, ev: Event) : (ConsensusState, Result) = {
    if (state.step == "precommit") {
        val result = { ...defaultResult, name: "timeout", timeout: "timeoutPrecommit" } // do we need the roundnumber here?
        (state, result)
    }
    else
        (state, defaultResult) 
}

// line 49
pure def ProposalAndCommitAndValid  (state: ConsensusState, ev: Event) : (ConsensusState, Result) = {
    if (state.step != "decided") {
        val newstate = { ...state, step: "decided"}
        val result = { ...defaultResult, name: "decided", decided: ev.value}
        (newstate, result)
    }
    else
        (state, defaultResult) 
} 

// line 55
pure def RoundSkip  (state: ConsensusState, ev: Event) : (ConsensusState, Result) = {
    if (ev.round > state.round)
        val result = { ...defaultResult, name: "skipRound", skipRound: ev.round }
        (state, result)
    else
        (state, defaultResult)
}

pure def TimeoutPropose (state: ConsensusState, ev: Event) : (ConsensusState, Result) = {
    if (ev.height == state.height and ev.round == state.round and state.step == "propose")
        val newstate = { ...state, step: "prevote"}
        val result = { ...defaultResult, name: "votemessage",
                                         voteMessage: { src: state.p,
                                                            height: state.height,
                                                            round: state.round,
                                                            step: "prevote",  // "prevote" or "precommit"
                                                            id: "nil"}}
        (newstate, result)
    else
        (state, defaultResult)
}

pure def TimeoutPrevote (state: ConsensusState, ev: Event) : (ConsensusState, Result) = {
    if (ev.height == state.height and ev.round == state.round and state.step == "prevote")
        val newstate = { ...state, step: "precommit"}
        val result = { ...defaultResult, name: "votemessage",
                                         voteMessage: { src: state.p,
                                                            height: state.height,
                                                            round: state.round,
                                                            step: "precommit",  // "prevote" or "precommit"
                                                            id: "nil"}}
        (newstate, result)
    else
        (state, defaultResult)
}    

pure def TimeoutPrecommit (state: ConsensusState, ev: Event) : (ConsensusState, Result) = {
    if (ev.height == state.height and ev.round == state.round)
        val result = {...defaultResult, name: "skipRound", skipRound: state.round + 1}
        (state, result)
    else
        (state, defaultResult)
}    


pure def consensus (state: ConsensusState, ev: Event) : (ConsensusState, Result) = {
    if (ev.name == "NewHeight")
        NewHeight (state, ev)
    else if (ev.name == "NewRoundProposer") 
        NewRoundProposer(state, ev)
    else if (ev.name == "NewRound")
        NewRound(state, ev)
    else if (ev.name == "Proposal")
        Proposal(state, ev)
    else if (ev.name == "ProposalAndPolkaPreviousAndValid")
        ProposalAndPolkaPreviousAndValid(state, ev)
    else if (ev.name == "ProposalInvalid")
        ProposalInvalid(state, ev)
    else if (ev.name == "PolkaAny")
        PolkaAny(state, ev)
    else if (ev.name == "ProposalAndPolkaAndValid")
        ProposalAndPolkaAndValid(state, ev)
    else if (ev.name == "PolkaNil")
        PolkaNil(state, ev)
    else if (ev.name == "PrecommitAny")
        PrecommitAny(state, ev)
    else if (ev.name == "ProposalAndCommitAndValid")
        ProposalAndCommitAndValid(state, ev)
    else if (ev.name == "TimeoutPropose")
        TimeoutPropose (state, ev)
    else if (ev.name == "TimeoutPrevote")
        TimeoutPrevote (state, ev)
    else if (ev.name == "TimeoutPrecommit")
        TimeoutPrecommit (state, ev)
    else
        (state, defaultResult)
}

/* ****************************************************************************
 * Global state
 * ************************************************************************* */

var system : Address_t -> ConsensusState
var _Result : Result
var _Event : Event


pure def initialProcess (name: Address_t) : ConsensusState = {
    { p: name, height : 1, round: 0, step: "newRound", lockedRound: -1, lockedValue: "nil", validRound: -1, validValue: "nil"}
}

action init = all {
    system' = Map ("Josef" -> initialProcess("Josef")),
    _Result' = defaultResult,
    _Event' = {name : "Initial",
                                            height : -1,
                                            round: -1,
                                            value: "",
                                            vr: -1}
}



// just to write a test.
action FireEvent(eventName: str, proc: Address_t, h: Height_t, r: Round_t, value: Value_t, vr: Round_t) : bool = all {
    val event =  {name : eventName,
                                            height : h,
                                            round: r,
                                            value: value,
                                            vr: vr}
    val res = consensus(system.get(proc),  event )
    all {
        system' = system.put(proc, res._1),
        _Result' = res._2,
        _Event' = event
    }
}

action step = any {
    nondet name = oneOf(consensusEvents)
    nondet height = 1//oneOf(1.to(4))
    nondet round = 0//oneOf(1.to(4))
    nondet value = oneOf(Set("block 1", "block 2", "block 3"))
    nondet vr = oneOf(Set(-1, 1, 2, 3, 4))
    FireEvent(name, "Josef", height, round, value, vr)
}

action unchangedAll = all {
    system' = system,
    _Result' = _Result,
    _Event' = _Event,
}


// This test should call each event at least once
run DecideNonProposerTest = {
    init
    .then(FireEvent("NewRound", "Josef", 1, 0, "", -1))
    .then(all{
        assert(_Result.timeout == "timeoutPropose"),
        unchangedAll
    })
    .then(FireEvent("Proposal", "Josef", 1, 0, "block", -1))
    .then(all{
        assert(_Result.voteMessage.step == "prevote" and _Result.voteMessage.id == "block"),
        unchangedAll
    })
    .then(FireEvent("ProposalAndPolkaAndValid", "Josef", 1, 0, "block", -1))
    .then(all{
        assert(_Result.voteMessage.step == "precommit" and _Result.voteMessage.id == "block"),
        unchangedAll
    })
    .then(FireEvent("ProposalAndCommitAndValid", "Josef", 1, 0, "block", -1))  
    .then(all{
        assert(_Result.decided == "block"),
        unchangedAll
    })
    .then(FireEvent("NewHeight", "Josef", system.get("Josef").height + 1, 0, "", -1))
    .then(all{
        assert(system.get("Josef").height == 2),
        unchangedAll
    })    
    .then(FireEvent("NewRoundProposer", "Josef", 2, 0, "nextBlock", -1))
    .then(all{
        assert(_Result.timeout != "timeoutPropose" and _Result.proposal.proposal == "nextBlock"),
        unchangedAll
    })
    .then(FireEvent("Proposal", "Josef", 2, 0, "nextBlock", -1)) // it is assumed that the proposer receives its own message
    .then(all{
        assert(_Result.voteMessage.step == "prevote" and system.get("Josef").step == "prevote"),
        unchangedAll
    })
    .then(FireEvent("PolkaAny", "Josef", 2, 0, "nextBlock", -1))
    .then(all{
        assert(_Result.timeout == "timeoutPrevote"),
        unchangedAll
    })
    .then(FireEvent("TimeoutPrevote", "Josef", 2, 0, "nextBlock", -1))
    .then(all{
        assert(_Result.voteMessage.step == "precommit" and _Result.voteMessage.id == "nil" and
               system.get("Josef").step == "precommit"),
        unchangedAll
    })
    .then(FireEvent("PrecommitAny", "Josef", 2, 0, "nextBlock", -1))
    .then(all{
        assert(_Result.timeout == "timeoutPrecommit"),
        unchangedAll
    })
    .then(FireEvent("TimeoutPrecommit", "Josef", 2, 0, "nextBlock", -1))
    .then(all{
        assert(_Result.skipRound == 1),
        unchangedAll
    })
    .then(FireEvent("NewRound", "Josef", 2, 1, "", -1))
    .then(all{
        assert(_Result.timeout == "timeoutPropose"),
        unchangedAll
    })
    .then(FireEvent("TimeoutPropose", "Josef", 2, 1, "nextBlock", -1))
    .then(all{
        assert(_Result.voteMessage.step == "prevote" and _Result.voteMessage.id == "nil" and
               system.get("Josef").step == "prevote"),
        unchangedAll
    })
    .then(FireEvent("PolkaNil", "Josef", 2, 1, "nextBlock", -1))
    .then(all{
        assert(_Result.voteMessage.step == "precommit" and _Result.voteMessage.id == "nil" and
               system.get("Josef").step == "precommit"),
        unchangedAll
    })
    .then(FireEvent("PrecommitAny", "Josef", 2, 1, "nextBlock", -1))
    .then(all{
        assert(_Result.timeout == "timeoutPrecommit"),
        unchangedAll
    })
    .then(FireEvent("TimeoutPrecommit", "Josef", 2, 1, "nextBlock", -1))
    .then(all{
        assert(_Result.skipRound == 2),
        unchangedAll
    })
    .then(FireEvent("NewRound", "Josef", 2, 2, "", -1))
    .then(all{
        assert(_Result.timeout == "timeoutPropose"),
        unchangedAll
    })
    .then(FireEvent("ProposalInvalid", "Josef", 2, 2, "", -1))
    .then(all{
        assert(_Result.voteMessage.step == "prevote" and _Result.voteMessage.id == "nil" and
               system.get("Josef").step == "prevote"),
        unchangedAll
    })
}


}

