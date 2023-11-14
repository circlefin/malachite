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
- test id(v): most likely we need to change the type of Value_t as Quint doesn't have
  string operators. Perhaps we make Value_t = int and then id(v) = -v
*/


module consensus {
    
  // a process address is just a string
  type Address_t = str
  // a value is also a string
  type Value_t = str
  // a round step is also a string
  type Step_t = str
  // a round is an integer
  type Round_t = int
  // a height is an integer
  type Height_t = int
  // timeours are identified by strings
  type Timeout_t = str

// the type of propose messages
type ProposeMsg_t = {
    src: Address_t,
    height: Height_t,
    round: Round_t,
    proposal: Value_t, // an actual value. All other values are id(proposal)
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
    height : Height_t,
    round: Round_t,
    step: Step_t, // "newRound", propose, Prevote, Precommit, decided
    lockedRound: Round_t,
    lockedValue: Value_t, // id("of a value")
    validRound: Round_t,
    validValue: Value_t, // id("of a value")
    //continue
}  

pure def initConsensusState (v: Address_t) : ConsensusState = {
    p: v, 
    round: -1, 
    step: "newRound", 
    height : 0,
    lockedRound: -1, 
    lockedValue: "nil", 
    validRound: -1, 
    validValue: "nil"
}

type Event = {
    name : str,
    height : Height_t,
    round: Round_t,
    value: Value_t,
    vr: Round_t
}    

// what is a good way to encode optionals? I do with default values
type ConsensusOutput = {
    name : str,    
    proposal: ProposeMsg_t,
    voteMessage: VoteMsg_t,
    timeout: Timeout_t,
    decided: Value_t,
    skipRound: Round_t
}

val consensusOutputs = Set (
    "proposal",
    "votemessage",
    "timeout",
    "decided",
    "skipRound"
)

type ConsResult = {
    cs: ConsensusState,
    out: ConsensusOutput,
//    pending: Set[ConsensusOutput], // TODO: not sure we need this
}

type ConsensusEvent = str

val ConsensusEvents = Set(
    "NewHeight",			// Setups the state-machine for a single-height execution
    "NewRound",              		// Start a new round, not as proposer.
    "NewRoundProposer", 		// Start a new round as proposer with the proposed Value.
    "Proposal",      			// Receive a proposal without associated valid round.
    "ProposalAndPolkaPreviousAndValid", // Receive a valid proposal with an associated valid round, attested by a a Polka(vr).
    "ProposalInvalid", 			// Receive an invalid proposal: L26 and L32 when valid(v) == false
    "PolkaNil",                		// Receive +2/3 prevotes for nil.
    "PolkaAny",                		// Receive +2/3 prevotes from different validators, not for the same value or nil.
    "ProposalAndPolkaAndValid", 	// Proposal and 2/3+ prevotes for the proposal: L36 when valid and step >= prevote
    "PrecommitAny",           		// Receive +2/3 precommits from different validators, not for the same value or nil.
    "ProposalAndCommitAndValid", 	// Proposal and 2/3+ commits for the proposal => decision
    "RoundSkip",               		// Receive +1/3 messages from different validators for a higher round.
    "TimeoutPropose",          		// Timeout waiting for proposal.
    "TimeoutPrevote",          		// Timeout waiting for prevotes for a value.
    "TimeoutPrecommit",       		// Timeout waiting for precommits for a value.
// found after Montebello
    "ProposalAndPolkaAndInValid" // TODO: Discuss what to do about it
)

/*    
    "PolkaValue(ValueId)",     // Receive +2/3 Prevotes for Value.
    "PrecommitValue(ValueId)", // Receive +2/3 Precommits for Value.
*/

val noProp : ProposeMsg_t = {src: "", height: -1, round: -1, proposal: "", validRound: -1}
val noVote : VoteMsg_t = {src: "", height: -1, round: -1, step: "", id: ""}
val noTimeout : Timeout_t = ""
val noDecided = ""
val noSkipRound : Round_t = -1
val defaultResult : ConsensusOutput = { 
                name: "",
                proposal: noProp, 
                voteMessage: noVote, 
                timeout: noTimeout, 
                decided: noDecided,
                skipRound: noSkipRound}



pure def NewHeight (state: ConsensusState, ev: Event) : ConsResult = {
    if (ev.height > state.height)
        val newstate = {    p: state.p, 
                            round: -1, 
                            // must be -1, as nothing should be done before a NewRound
                            // function is called
                            step: "newRound", 
                            height : ev.height,
                            lockedRound: -1, 
                            lockedValue: "nil", 
                            validRound: -1, 
                            validValue: "nil"
                        }
        {cs: newstate, out: defaultResult }
    else
        {cs: state,  out: defaultResult }
}

// line 11.14
pure def NewRoundProposer (state: ConsensusState, ev: Event) : ConsResult = {
    if (ev.round > state.round)
        val newstate = { ...state, round: ev.round, step: "propose"}
        val proposal =  if (state.validValue != "nil") state.validValue
                        else ev.value
        val result = { ...defaultResult, name: "proposal",
                                         proposal: {   src: state.p, 
                                                            height: state.height,
                                                            round: ev.round, 
                                                            proposal: proposal,
                                                            validRound: state.validRound}}
        {cs: newstate, out: result }
    else
        {cs: state,  out: defaultResult }
}

// line 11.20
pure def NewRound (state: ConsensusState, ev: Event) : ConsResult = {
        // TODO: discuss comment "ev.round must match state.round"
    if (ev.round > state.round)        
        val newstate = { ...state, round: ev.round, step: "propose" }
        val result = { ...defaultResult, name: "timeout", timeout: "TimeoutPropose"} 
            // We just report that a timeout should be started. The executor must take care
            // of figuring out whether it needs to record the round number and height per
            // timeout
            {cs: newstate, out: result} 
    else
        {cs: state,  out: defaultResult }         
}

// line 22
// Here it is assumed that 
// - the value has been checked to be valid
// - it is for the current round
// The executor checks this upon receiving a propose message "ProposalMsg"
pure def Proposal (state: ConsensusState, ev: Event) : ConsResult = {
    if (state.step == "propose")
        val newstate = { ...state, step: "Prevote" }
        if (state.lockedRound == -1 or state.lockedValue == ev.value)
            val result = { ...defaultResult, name: "votemessage",
                                             voteMessage: { src: state.p,
                                                            height: state.height,            
                                                            round: state.round,
                                                            step: "Prevote",
                                                            id: ev.value}}
            {cs: newstate, out: result}
        else
            val result = { ...defaultResult, name: "votemessage",
                                             voteMessage: { src: state.p,
                                                            height: state.height,            
                                                            round: state.round,
                                                            step: "Prevote",
                                                            id: "nil"}}
            {cs: newstate, out: result}
    else
        {cs: state, out: defaultResult}
        // This should be dead code as the executor checks the step
}

// line 26
pure def ProposalInvalid (state: ConsensusState, ev: Event) : ConsResult = {
    if (state.step == "propose") 
        val newstate = state.with("step", "Prevote")
        val result = { ...defaultResult, name: "votemessage",
                                         voteMessage: { src: state.p,
                                                        height: state.height,
                                                        round: state.round,
                                                        step: "Prevote",
                                                        id: "nil"}}
        {cs: newstate, out: result}
    else
        {cs: state, out: defaultResult}     
}

// line 28
pure def ProposalAndPolkaPreviousAndValid (state: ConsensusState, ev: Event) : ConsResult = {
    if (state.step == "propose" and ev.vr >= 0 and ev.vr < state.round) 
        val newstate = state.with("step", "Prevote")
        if (state.lockedRound <= ev.vr or state.lockedValue == ev.value)
            val result = { ...defaultResult, name: "votemessage",
                                             voteMessage: { src: state.p,
                                                            height: state.height,
                                                            round: state.round,
                                                            step: "Prevote",
                                                            id: ev.value}}
            {cs: newstate, out: result}
        else
            val result = { ...defaultResult, name: "votemessage",
                                             voteMessage: { src: state.p,
                                                            height: state.height,
                                                            round: state.round,
                                                            step: "Prevote",
                                                            id: "nil"}}
            {cs: newstate, out: result}                                          
    else
        {cs: state, out: defaultResult}
        // TODO: should we add the event to pending in this case. We would need to
        // do this in the executor
}

// line 34
pure def PolkaAny (state: ConsensusState, ev: Event) : ConsResult = {
    if (state.step == "Prevote") 
        val result = { ...defaultResult, name: "timeout", timeout: "TimeoutPrevote" }
            // We just report that a timeout should be started. The executor must take care
            // of figuring out whether it needs to record the round number and height per
            // timeout
        {cs: state, out: result}
    else
        {cs: state, out: defaultResult}
}

// line 36
pure def ProposalAndPolkaAndValid (state: ConsensusState, ev: Event) : ConsResult = {
    val auxState = { ...state, validValue: ev.value, validRound: state.round }
    if (state.step == "Prevote") 
        val newstate = { ...auxState,   lockedValue: ev.value,
                                        lockedRound: state.round,
                                        step: "Precommit" }
        val result = { ...defaultResult, name: "votemessage",
                                         voteMessage: { src: state.p,
                                                        height: state.height,
                                                        round: state.round,
                                                        step: "Precommit",  
                                                        id: ev.value}}
        {cs: newstate, out: result}
    else if (state.step == "Precommit")
        // TODO: check whether Daniel's comment
        // "if state > prevote, we should update the valid round!"
        // was properly addressed
        {cs: auxState,  out: defaultResult}
    else
        {cs: state, out: defaultResult}
        // TODO: should we add the event to pending in this case. We would need to
        // do this in the executor
}

// line 44
pure def PolkaNil (state: ConsensusState, ev: Event) : ConsResult = {
    if (state.step == "Prevote")
        val newstate = { ...state, step: "Precommit"}
        val result = { ...defaultResult, name: "votemessage",
                                         voteMessage: { src: state.p,
                                                        height: state.height,
                                                        round: state.round,
                                                        step: "Precommit",  
                                                        id: "nil"}}
        {cs: newstate, out: result}
    else
        {cs: state, out: defaultResult}
}

// line 47
pure def PrecommitAny (state: ConsensusState, ev: Event) : ConsResult = {
    val result = { ...defaultResult, name: "timeout", timeout: "TimeoutPrecommit" }
    {cs: state, out: result}
}

// line 49
pure def ProposalAndCommitAndValid  (state: ConsensusState, ev: Event) : ConsResult = {
    if (state.step != "decided") {
        val newstate = { ...state, step: "decided"}
        val result = { ...defaultResult, name: "decided", decided: ev.value}
        {cs: newstate, out: result}
    }
    else
        {cs: state, out: defaultResult} 
}

// line 55
pure def RoundSkip  (state: ConsensusState, ev: Event) : ConsResult = {
    if (ev.round > state.round)
        val result = { ...defaultResult, name: "skipRound", skipRound: ev.round }
        {cs: state, out: result}
    else
        {cs: state, out: defaultResult}
}

pure def TimeoutPropose (state: ConsensusState, ev: Event) : ConsResult = {
    if (ev.height == state.height and ev.round == state.round and state.step == "propose")
        val newstate = { ...state, step: "Prevote"}
        val result = { ...defaultResult, name: "votemessage",
                                         voteMessage: { src: state.p,
                                                        height: state.height,
                                                        round: state.round,
                                                        step: "Prevote",
                                                        id: "nil"}}
        {cs: newstate, out: result}
    else
        {cs: state, out: defaultResult}
}

pure def TimeoutPrevote (state: ConsensusState, ev: Event) : ConsResult = {
    if (ev.height == state.height and ev.round == state.round and state.step == "Prevote")
        val newstate = { ...state, step: "Precommit"}
        // TODO: should we send precommit nil again ?
        val result = { ...defaultResult, name: "votemessage",
                                         voteMessage: { src: state.p,
                                                        height: state.height,
                                                        round: state.round,
                                                        step: "Precommit", 
                                                        id: "nil"}}
        {cs: newstate, out: result}
    else
        {cs: state, out: defaultResult}
}    

pure def TimeoutPrecommit (state: ConsensusState, ev: Event) : ConsResult = {
    if (ev.height == state.height and ev.round == state.round)
        // TODO: here we should call newRound. For this we would need to know whether
        // we are proposer for next round.
        val result = {...defaultResult, name: "skipRound", skipRound: state.round + 1}
        {cs: state, out: result}
    else
        {cs: state, out: defaultResult}
}    


/* *********************************************************
 * Main entry point
 * ********************************************************/

pure def consensus (state: ConsensusState, ev: Event) : ConsResult = {
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
        {cs: state, out: defaultResult}
}

}

