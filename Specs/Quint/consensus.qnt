// -*- mode: Bluespec; -*-

/*
TODO: check 
- whether we have "step" checks in place everywhere
- "the first time": checks here or in executor
- check against arXiv
- tests
- types (e.g., heights in the messages)
- discuss "decision tree" in executor
- Should we think again about the components and the boundaries (especially between
  voteBookkeeper and executor)
- Do we need tests for executor and bookkeeping
*/


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
    step: Step_t,  // "Prevote" or "Precommit"
    id: Value_t,
  }

type ConsensusState = {
    p: Address_t,
    height : int,
    round: Round_t,
    step: Step_t, // "newRound", propose, Prevote, Precommit, decided
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

val consensusResults = Set (
    "proposal",
    "votemessage",
    "timeout",
    "decided",
    "skipRound"
)

val consensusEvents = Set(
    "NewHeight",
    "NewRound",                // Start a new round, not as proposer.
    "NewRoundProposer", // Start a new round and propose the Value.
    "Proposal",      // Receive a proposal with possible polka round.
    "ProposalAndPolkaPreviousAndValid", //28 when valid
    "ProposalInvalid", // 26 and 32 when invalid in step propose
    "PolkaNil",                // Receive +2/3 Prevotes for nil.
    "PolkaAny",                // Receive +2/3 Prevotes for anything and they are not the same
    "ProposalAndPolkaAndValid", // 36 when valid and step >= Prevote
    "PrecommitAny",           // Receive +2/3 Precommits for anything.
    "ProposalAndCommitAndValid", // decide
    "RoundSkip",               // Receive +1/3 votes from a higher round.
    "TimeoutPropose",          // Timeout waiting for proposal.
    "TimeoutPrevote",          // Timeout waiting for Prevotes.
    "TimeoutPrecommit",       // Timeout waiting for Precommits
// found after Montebello
    "ProposalAndPolkaAndInValid"

)

/*    

    "PolkaValue(ValueId)",     // Receive +2/3 Prevotes for Value.
    "PrecommitValue(ValueId)", // Receive +2/3 Precommits for Value.

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
    val newstate = {        p: state.p, 
                            round: ev.round, 
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
    val result = { ...defaultResult, name: "timeout", timeout: "TimeoutPropose"} // do we need the roundnumber here?
        (newstate, result)
}

// line 22
pure def Proposal (state: ConsensusState, ev: Event) : (ConsensusState, Result) = {
    if (state.step == "propose") {
        val newstate = state.with("step", "Prevote")
        if (state.lockedRound == -1 or state.lockedValue == ev.value)
            val result = { ...defaultResult, name: "votemessage",
                                             voteMessage: { src: state.p,
                                                            height: state.height,            
                                                            round: state.round,
                                                            step: "Prevote",  // "Prevote" or "Precommit"
                                                            id: ev.value}}
            (newstate, result)
        else
            val result = { ...defaultResult, name: "votemessage",
                                             voteMessage: { src: state.p,
                                                            height: state.height,            
                                                            round: state.round,
                                                            step: "Prevote",  // "Prevote" or "Precommit"
                                                            id: "nil"}}
            (newstate, result)
    }
    else
        (state, defaultResult)
}

// line 28
pure def ProposalAndPolkaPreviousAndValid (state: ConsensusState, ev: Event) : (ConsensusState, Result) = {
    if (state.step == "propose" and ev.vr >= 0 and ev.vr < state.round) {
        val newstate = state.with("step", "Prevote")
        if (state.lockedRound <= ev.vr or state.lockedValue == ev.value)
            val result = { ...defaultResult, name: "votemessage",
                                             voteMessage: { src: state.p,
                                                            height: state.height,
                                                            round: state.round,
                                                            step: "Prevote",  // "Prevote" or "Precommit"
                                                            id: ev.value}}
            (newstate, result)
        else
            val result = { ...defaultResult, name: "votemessage",
                                             voteMessage: { src: state.p,
                                                            height: state.height,
                                                            round: state.round,
                                                            step: "Prevote",  // "Prevote" or "Precommit"
                                                            id: "nil"}}
            (newstate, result)                                             
    }
    else
        (state, defaultResult)
}

pure def ProposalInvalid (state: ConsensusState, ev: Event) : (ConsensusState, Result) = {
    if (state.step == "propose") {
        val newstate = state.with("step", "Prevote")
        val result = { ...defaultResult, name: "votemessage",
                                         voteMessage: { src: state.p,
                                                            height: state.height,
                                                            round: state.round,
                                                            step: "Prevote",  // "Prevote" or "Precommit"
                                                            id: "nil"}}
        (newstate, result)
    }
    else {
        (state, defaultResult )      
    }
}

// line 34
pure def PolkaAny (state: ConsensusState, ev: Event) : (ConsensusState, Result) = {
    if (state.step == "Prevote") {
        val result = { ...defaultResult, name: "timeout", timeout: "TimeoutPrevote" } // do we need the roundnumber here?
        (state, result)
    }
    else
        (state, defaultResult) 
}

// line 36
pure def ProposalAndPolkaAndValid (state: ConsensusState, ev: Event) : (ConsensusState, Result) = {
    val auxState = { ...state, validValue: ev.value, validRound: state.round }
    if (state.step == "Prevote") {
        val newstate = { ...auxState,   lockedValue: ev.value,
                                        lockedRound: state.round,
                                        step: "Precommit" }
        val result = { ...defaultResult, name: "votemessage",
                                         voteMessage: { src: state.p,
                                                        height: state.height,
                                                        round: state.round,
                                                        step: "Precommit",  
                                                        id: ev.value}}
        (newstate, result)
        }
    else {
        (state, defaultResult)
    }
}

// line 44
pure def PolkaNil (state: ConsensusState, ev: Event) : (ConsensusState, Result) = {
    if (state.step != "Prevote")
        (state, defaultResult)
    else
        val newstate = { ...state, step: "Precommit"}
        val result = { ...defaultResult, name: "votemessage",
                                         voteMessage: { src: state.p,
                                                        height: state.height,
                                                        round: state.round,
                                                        step: "Precommit",  
                                                        id: "nil"}}
        (newstate, result)
}

// line 47
pure def PrecommitAny (state: ConsensusState, ev: Event) : (ConsensusState, Result) = {
    if (state.step == "Precommit") {
        val result = { ...defaultResult, name: "timeout", timeout: "TimeoutPrecommit" } // do we need the roundnumber here?
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
        val newstate = { ...state, step: "Prevote"}
        val result = { ...defaultResult, name: "votemessage",
                                         voteMessage: { src: state.p,
                                                            height: state.height,
                                                            round: state.round,
                                                            step: "Prevote",  // "Prevote" or "Precommit"
                                                            id: "nil"}}
        (newstate, result)
    else
        (state, defaultResult)
}

pure def TimeoutPrevote (state: ConsensusState, ev: Event) : (ConsensusState, Result) = {
    if (ev.height == state.height and ev.round == state.round and state.step == "Prevote")
        val newstate = { ...state, step: "Precommit"}
        val result = { ...defaultResult, name: "votemessage",
                                         voteMessage: { src: state.p,
                                                            height: state.height,
                                                            round: state.round,
                                                            step: "Precommit",  // "Prevote" or "Precommit"
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



}

