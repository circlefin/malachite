// -*- mode: Bluespec; -*-

/*
TODO: check 
- whether we have "step" checks in place everywhere
- "the first time": checks here or in driver
- check against arXiv
- tests
- types (e.g., heights in the messages)
- discuss "decision tree" in driver
- Should we think again about the components and the boundaries (especially between
  voteBookkeeper and driver)
- Do we need tests for driver and bookkeeping
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
type Proposal_t = {
    src: Address_t,
    height: Height_t,
    round: Round_t,
    proposal: Value_t, // an actual value. All other values are id(proposal)
    validRound: Round_t
}

// the type of Prevote and Precommit messages
type Vote_t = {
    src: Address_t,
    height: Height_t,
    round: Round_t,
    step: Step_t,  // "Prevote" or "Precommit"
    id: Value_t,
}

type ConsensusState = {
    p: Address_t,
    height: Height_t,
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
    height: 0,
    lockedRound: -1, 
    lockedValue: "nil", 
    validRound: -1, 
    validValue: "nil"
}

type ConsensusInput = {
    name: str,
    height : Height_t,
    round: Round_t,
    value: Value_t,
    vr: Round_t
}    

// what is a good way to encode optionals? I do with default values
type ConsensusOutput = {
    name: str,    
    proposal: Proposal_t,
    voteMessage: Vote_t,
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

val ConsensusInputNames = Set(
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

val noProp : Proposal_t = {src: "", height: -1, round: -1, proposal: "", validRound: -1}
val noVote : Vote_t = {src: "", height: -1, round: -1, step: "", id: ""}
val noTimeout : Timeout_t = ""
val noDecided = ""
val noSkipRound : Round_t = -1
val defaultOutput : ConsensusOutput = { 
                name: "",
                proposal: noProp, 
                voteMessage: noVote, 
                timeout: noTimeout, 
                decided: noDecided,
                skipRound: noSkipRound}



pure def NewHeight (state: ConsensusState, input: ConsensusInput) : ConsResult = {
    if (input.height > state.height)
        val newstate = {    p: state.p, 
                            round: -1, 
                            // must be -1, as nothing should be done before a NewRound
                            // function is called
                            step: "newRound", 
                            height : input.height,
                            lockedRound: -1, 
                            lockedValue: "nil", 
                            validRound: -1, 
                            validValue: "nil"
                        }
        {cs: newstate, out: defaultOutput }
    else
        {cs: state,  out: defaultOutput }
}

// line 11.14
pure def NewRoundProposer (state: ConsensusState, input: ConsensusInput) : ConsResult = {
    if (input.round > state.round)
        val newstate = { ...state, round: input.round, step: "propose"}
        val proposal =  if (state.validValue != "nil") state.validValue
                        else input.value
        val result = { ...defaultOutput, name: "proposal",
                                         proposal: {   src: state.p, 
                                                            height: state.height,
                                                            round: input.round, 
                                                            proposal: proposal,
                                                            validRound: state.validRound}}
        {cs: newstate, out: result }
    else
        {cs: state,  out: defaultOutput }
}

// line 11.20
pure def NewRound (state: ConsensusState, input: ConsensusInput) : ConsResult = {
        // TODO: discuss comment "input.round must match state.round"
    if (input.round > state.round)        
        val newstate = { ...state, round: input.round, step: "propose" }
        val result = { ...defaultOutput, name: "timeout", timeout: "TimeoutPropose"} 
            // We just report that a timeout should be started. The driver must take care
            // of figuring out whether it needs to record the round number and height per
            // timeout
            {cs: newstate, out: result} 
    else
        {cs: state,  out: defaultOutput }         
}

// line 22
// Here it is assumed that 
// - the value has been checked to be valid
// - it is for the current round
// The driver checks this upon receiving a propose message "ProposalMsg"
pure def Proposal (state: ConsensusState, input: ConsensusInput) : ConsResult = {
    if (state.step == "propose")
        val newstate = { ...state, step: "Prevote" }
        if (state.lockedRound == -1 or state.lockedValue == input.value)
            val result = { ...defaultOutput, name: "votemessage",
                                             voteMessage: { src: state.p,
                                                            height: state.height,            
                                                            round: state.round,
                                                            step: "Prevote",
                                                            id: input.value}}
            {cs: newstate, out: result}
        else
            val result = { ...defaultOutput, name: "votemessage",
                                             voteMessage: { src: state.p,
                                                            height: state.height,            
                                                            round: state.round,
                                                            step: "Prevote",
                                                            id: "nil"}}
            {cs: newstate, out: result}
    else
        {cs: state, out: defaultOutput}
        // This should be dead code as the driver checks the step
}

// line 26
pure def ProposalInvalid (state: ConsensusState, input: ConsensusInput) : ConsResult = {
    if (state.step == "propose") 
        val newstate = state.with("step", "Prevote")
        val result = { ...defaultOutput, name: "votemessage",
                                         voteMessage: { src: state.p,
                                                        height: state.height,
                                                        round: state.round,
                                                        step: "Prevote",
                                                        id: "nil"}}
        {cs: newstate, out: result}
    else
        {cs: state, out: defaultOutput}     
}

// line 28
pure def ProposalAndPolkaPreviousAndValid (state: ConsensusState, input: ConsensusInput) : ConsResult = {
    if (state.step == "propose" and input.vr >= 0 and input.vr < state.round) 
        val newstate = state.with("step", "Prevote")
        if (state.lockedRound <= input.vr or state.lockedValue == input.value)
            val result = { ...defaultOutput, name: "votemessage",
                                             voteMessage: { src: state.p,
                                                            height: state.height,
                                                            round: state.round,
                                                            step: "Prevote",
                                                            id: input.value}}
            {cs: newstate, out: result}
        else
            val result = { ...defaultOutput, name: "votemessage",
                                             voteMessage: { src: state.p,
                                                            height: state.height,
                                                            round: state.round,
                                                            step: "Prevote",
                                                            id: "nil"}}
            {cs: newstate, out: result}                                          
    else
        {cs: state, out: defaultOutput}
        // TODO: should we add the event to pending in this case. We would need to
        // do this in the driver
}

// line 34
pure def PolkaAny (state: ConsensusState, input: ConsensusInput) : ConsResult = {
    if (state.step == "Prevote") 
        val result = { ...defaultOutput, name: "timeout", timeout: "TimeoutPrevote" }
            // We just report that a timeout should be started. The driver must take care
            // of figuring out whether it needs to record the round number and height per
            // timeout
        {cs: state, out: result}
    else
        {cs: state, out: defaultOutput}
}

// line 36
pure def ProposalAndPolkaAndValid (state: ConsensusState, input: ConsensusInput) : ConsResult = {
    val auxState = { ...state, validValue: input.value, validRound: state.round }
    if (state.step == "Prevote") 
        val newstate = { ...auxState,   lockedValue: input.value,
                                        lockedRound: state.round,
                                        step: "Precommit" }
        val result = { ...defaultOutput, name: "votemessage",
                                         voteMessage: { src: state.p,
                                                        height: state.height,
                                                        round: state.round,
                                                        step: "Precommit",  
                                                        id: input.value}}
        {cs: newstate, out: result}
    else if (state.step == "Precommit")
        // TODO: check whether Daniel's comment
        // "if state > prevote, we should update the valid round!"
        // was properly addressed
        {cs: auxState,  out: defaultOutput}
    else
        {cs: state, out: defaultOutput}
        // TODO: should we add the event to pending in this case. We would need to
        // do this in the driver
}

// line 44
pure def PolkaNil (state: ConsensusState, input: ConsensusInput) : ConsResult = {
    if (state.step == "Prevote")
        val newstate = { ...state, step: "Precommit"}
        val result = { ...defaultOutput, name: "votemessage",
                                         voteMessage: { src: state.p,
                                                        height: state.height,
                                                        round: state.round,
                                                        step: "Precommit",  
                                                        id: "nil"}}
        {cs: newstate, out: result}
    else
        {cs: state, out: defaultOutput}
}

// line 47
pure def PrecommitAny (state: ConsensusState, input: ConsensusInput) : ConsResult = {
    val result = { ...defaultOutput, name: "timeout", timeout: "TimeoutPrecommit" }
    {cs: state, out: result}
}

// line 49
pure def ProposalAndCommitAndValid  (state: ConsensusState, input: ConsensusInput) : ConsResult = {
    if (state.step != "decided") {
        val newstate = { ...state, step: "decided"}
        val result = { ...defaultOutput, name: "decided", decided: input.value}
        {cs: newstate, out: result}
    }
    else
        {cs: state, out: defaultOutput} 
}

// line 55
pure def RoundSkip  (state: ConsensusState, input: ConsensusInput) : ConsResult = {
    if (input.round > state.round)
        val result = { ...defaultOutput, name: "skipRound", skipRound: input.round }
        {cs: state, out: result}
    else
        {cs: state, out: defaultOutput}
}

pure def TimeoutPropose (state: ConsensusState, input: ConsensusInput) : ConsResult = {
    if (input.height == state.height and input.round == state.round and state.step == "propose")
        val newstate = { ...state, step: "Prevote"}
        val result = { ...defaultOutput, name: "votemessage",
                                         voteMessage: { src: state.p,
                                                        height: state.height,
                                                        round: state.round,
                                                        step: "Prevote",
                                                        id: "nil"}}
        {cs: newstate, out: result}
    else
        {cs: state, out: defaultOutput}
}

pure def TimeoutPrevote (state: ConsensusState, input: ConsensusInput) : ConsResult = {
    if (input.height == state.height and input.round == state.round and state.step == "Prevote")
        val newstate = { ...state, step: "Precommit"}
        // TODO: should we send precommit nil again ?
        val result = { ...defaultOutput, name: "votemessage",
                                         voteMessage: { src: state.p,
                                                        height: state.height,
                                                        round: state.round,
                                                        step: "Precommit", 
                                                        id: "nil"}}
        {cs: newstate, out: result}
    else
        {cs: state, out: defaultOutput}
}    

pure def TimeoutPrecommit (state: ConsensusState, input: ConsensusInput) : ConsResult = {
    if (input.height == state.height and input.round == state.round)
        // TODO: here we should call newRound. For this we would need to know whether
        // we are proposer for next round.
        val result = {...defaultOutput, name: "skipRound", skipRound: state.round + 1}
        {cs: state, out: result}
    else
        {cs: state, out: defaultOutput}
}    


/* *********************************************************
 * Main entry point
 * ********************************************************/

pure def consensus (state: ConsensusState, input: ConsensusInput) : ConsResult = {
    if (input.name == "NewHeight")
        NewHeight (state, input)
    else if (input.name == "NewRoundProposer") 
        NewRoundProposer(state, input)
    else if (input.name == "NewRound")
        NewRound(state, input)
    else if (input.name == "Proposal")
        Proposal(state, input)
    else if (input.name == "ProposalAndPolkaPreviousAndValid")
        ProposalAndPolkaPreviousAndValid(state, input)
    else if (input.name == "ProposalInvalid")
        ProposalInvalid(state, input)
    else if (input.name == "PolkaAny")
        PolkaAny(state, input)
    else if (input.name == "ProposalAndPolkaAndValid")
        ProposalAndPolkaAndValid(state, input)
    else if (input.name == "PolkaNil")
        PolkaNil(state, input)
    else if (input.name == "PrecommitAny")
        PrecommitAny(state, input)
    else if (input.name == "ProposalAndCommitAndValid")
        ProposalAndCommitAndValid(state, input)
    else if (input.name == "RoundSkip")
        RoundSkip (state, input)
    else if (input.name == "TimeoutPropose")
        TimeoutPropose (state, input)
    else if (input.name == "TimeoutPrevote")
        TimeoutPrevote (state, input)
    else if (input.name == "TimeoutPrecommit")
        TimeoutPrecommit (state, input)
    else
        {cs: state, out: defaultOutput}
}

}

