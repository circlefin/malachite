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
- test id(v): most likely we need to change the type of Value as Quint doesn't have
  string operators. Perhaps we make Value = int and then id(v) = -v
*/

module consensus {

    import types.* from "./types"

    // *************************************************************************
    // Consensus state
    // *************************************************************************

    type ConsensusState = {
        p: Address,
        height : Height,
        round: Round,
        step: Step, // NewRound, Propose, Prevote, Precommit, decided
        lockedRound: Round,
        lockedValue: ValueId,
        validRound: Round,
        validValue: ValueId,
        //continue
    }  

    pure def initConsensusState(p: Address, h: Height) : ConsensusState = {
        p: p, 
        // round must be -1, as nothing should be done before a NewRound
        // function is called
        round: -1, 
        step: NewRoundStep, 
        height: h,
        lockedRound: -1, 
        lockedValue: Nil, 
        validRound: -1, 
        validValue: Nil
    }

    // *************************************************************************
    // Consensus input
    // *************************************************************************

    type ConsensusInput = 
        // For the initial state
        | NoConsensusInput
        // Setup the state-machine for a single-height execution
        | NewHeightConInput(Height)
        // Start a new round, not as proposer.
        | NewRoundConInput(Round)
        // Start a new round as proposer with the proposed Value.
        | NewRoundProposerConInput((Round, NonNilValue))
        // Receive a proposal without associated valid round.
        | ProposalConInput((Round, Value))
        // Receive a valid proposal with an associated valid round, attested by a a Polka(vr).
        | ProposalAndPolkaPreviousAndValidConInput((Value, Round))
        // Receive an invalid proposal: L26 and L32 when isValid(v) == false
        | ProposalInvalidConInput
        // Receive +2/3 prevotes for nil.
        | PolkaNilConInput
        // Receive +2/3 prevotes from different validators, not for the same value or nil.
        | PolkaAnyConInput
        // Proposal and 2/3+ prevotes for the proposal: L36 when valid and step >= prevote
        | ProposalAndPolkaAndValidConInput(Value)
        // Receive +2/3 precommits from different validators, not for the same value or nil.
        | PrecommitAnyConInput
        // Proposal and 2/3+ commits for the proposal => decision
        | ProposalAndCommitAndValidConInput(Value)
        // Receive +1/3 messages from different validators for a higher round.
        | RoundSkipConInput(Round)
        // Timeout waiting for proposal.
        | TimeoutProposeConInput((Height, Round))
        // Timeout waiting for prevotes for a value.
        | TimeoutPrevoteConInput((Height, Round))
        // Timeout waiting for precommits for a value.
        | TimeoutPrecommitConInput((Height, Round))
        // found after Montebello
        // TODO: Discuss what to do about it
        | ProposalAndPolkaAndInvalidConInput((Height, Round, Value))

    // *************************************************************************
    // Consensus output
    // *************************************************************************

    type ConsensusOutput =
        | NoConsensusOutput
        | ProposalOutput(Proposal)
        | VoteMessageOutput(Vote)
        | TimeoutOutput(Timeout)
        | DecidedOutput(Value)
        | SkipRoundOutput(Round)
        | ErrorOutput(str)

    pure def toNoConsensusOutput(state) = 
        { cs: state, out: NoConsensusOutput }
    pure def toProposalOutput(state, src, height, round, value, validRound) = 
        { cs: state, out: ProposalOutput(mkProposal(src, height, round, value, validRound)) }
    pure def toPrevoteOutput(state, src, height, round, valueId) = 
        { cs: state, out: VoteMessageOutput(mkVote(Prevote, src, height, round, valueId)) }
    pure def toPrecommitOutput(state, src, height, round, valueId) = 
        { cs: state, out: VoteMessageOutput(mkVote(Precommit, src, height, round, valueId)) }
    pure def toTimeoutOutput(state, timeout) = 
        { cs: state, out: TimeoutOutput(timeout) }
    pure def toDecidedOutput(state, value) = 
        { cs: state, out: DecidedOutput(value) }
    pure def toSkipRoundOutput(state, round) = 
        { cs: state, out: SkipRoundOutput(round) }
    pure def toErrorOutput(state, msg) = 
        { cs: state, out: ErrorOutput(msg) }

    // for testing
    pure def isVoteMsgWith(output, voteType, valueId) =
        match output {
            | VoteMessageOutput(msg) => msg.voteType == voteType and msg.valueId == valueId
            | _ => false
        }

    // *************************************************************************
    // Result = new state + output
    // *************************************************************************

    type ConsensusResult = {
        cs: ConsensusState,
        out: ConsensusOutput,
    //    pending: Set[ConsensusOutput], // TODO: not sure we need this
    }

    // *************************************************************************
    // State transition
    // *************************************************************************

    pure def consensus(state: ConsensusState, input: ConsensusInput): ConsensusResult =
        match input {
        | NoConsensusInput => 
            state.toErrorOutput("NoConsensusInput is not a valid consensus input")
        
        | NewHeightConInput(h) => 
            if (h > state.height)
                initConsensusState(state.p, h).toNoConsensusOutput()
            else state.toNoConsensusOutput()
        
        // line 11.14
        | NewRoundProposerConInput(round_value) =>
            val r = round_value._1
            val v = round_value._2
            if (r > state.round)
                val proposedValue = if (state.validValue != Nil) state.validValue.getVal() else v // lines 15-18
                state
                    .with("step", ProposeStep)
                    .with("round", r)
                    .toProposalOutput(state.p, state.height, r, proposedValue, state.validRound) // line 19
            else state.toErrorOutput("new round is not bigger than current round")

        // line 11.20
        // TODO: discuss comment "ev.round must match state.round"
        | NewRoundConInput(r) =>
            if (r > state.round)
                // We just report that a timeout should be started. The executor must take care
                // of figuring out whether it needs to record the round number and height per
                // timeout
                state
                    .with("step", ProposeStep)
                    .with("round", r)
                    .toTimeoutOutput(ProposeTimeout) // line 21
            else state.toErrorOutput("new round is not bigger than current round")

        // line 22
        // Here it is assumed that 
        // - the value has been checked to be valid
        // - it is for the current round
        // The executor checks this upon receiving a propose message "ProposalMsg"
        | ProposalConInput(round_value) =>
            val r = round_value._1
            val v = round_value._2
            if (state.step == ProposeStep)
                if (state.lockedRound == -1 or state.lockedValue == v) // line 23
                    state
                        .with("step", PrevoteStep)
                        .toPrevoteOutput(state.p, state.height, state.round, id(v)) // line 24
                else
                    state
                        .with("step", PrevoteStep)
                        .toPrevoteOutput(state.p, state.height, state.round, Nil) // line 26
            else state.toErrorOutput("ProposalConInput when not in ProposeStep")

        // line 26
        | ProposalInvalidConInput =>
            if (state.step == ProposeStep) 
                state
                    .with("step", PrevoteStep)
                    .toPrevoteOutput(state.p, state.height, state.round, Nil)
            else state.toErrorOutput("ProposalConInput when not in ProposeStep")

        // line 28
        | ProposalAndPolkaPreviousAndValidConInput(value_vr) =>
            val v = value_vr._1
            val vr = value_vr._2
            if (state.step == ProposeStep and vr >= 0 and vr < state.round) // line 28
                if (state.lockedRound <= vr or state.lockedValue == v) // line 29
                    state
                        .with("step", PrevoteStep) // line 33
                        .toPrevoteOutput(state.p, state.height, state.round, id(v)) // line 30
                else 
                    state
                        .with("step", PrevoteStep) // line 33
                        .toPrevoteOutput(state.p, state.height, state.round, Nil) // line 32
            else state.toErrorOutput("ProposalAndPolkaPreviousAndValidConInput")
                // TODO: should we add the event to pending in this case. We would need to
                // do this in the executor

        // line 34
        | PolkaAnyConInput =>
            if (state.step == PrevoteStep)
                // We just report that a timeout should be started. The executor must take care
                // of figuring out whether it needs to record the round number and height per
                // timeout
                state.toTimeoutOutput(PrevoteTimeout)
            else state.toErrorOutput("PolkaAnyConInput when not in PrevoteStep")

        // line 36
        | ProposalAndPolkaAndValidConInput(v) =>
            match state.step {
                | PrevoteStep =>
                    state
                        .with("step", PrecommitStep) // line 41
                        .with("lockedValue", v) // line 38
                        .with("lockedRound", state.round) // line 39
                        .with("validValue", v) // line 42
                        .with("validRound", state.round) // line 43
                        .toPrecommitOutput(state.p, state.height, state.round, id(v)) // line 40
                | PrecommitStep =>
                    // TODO: check whether Daniel's comment
                    // "if state > prevote, we should update the valid round!"
                    // was properly addressed
                    state
                        .with("validValue", v) // line 42
                        .with("validRound", state.round) // line 43
                        .toNoConsensusOutput()
                | _ => state.toErrorOutput("ProposalAndPolkaAndValidConInput when not in PrevoteStep or PrecommitStep")
            }

        // line 44
        | PolkaNilConInput => 
            if (state.step == PrevoteStep)
                state
                    .with("step", PrecommitStep)
                    .toPrecommitOutput(state.p, state.height, state.round, Nil)
            else state.toErrorOutput("PolkaNilConInput when not in PrevoteStep")

        // line 47
        | PrecommitAnyConInput =>
            state.toTimeoutOutput(PrecommitTimeout)

        // line 49
        | ProposalAndCommitAndValidConInput(v) => 
            if (state.step != DecidedStep)
                state
                    .with("step", DecidedStep)
                    .toDecidedOutput(v)
            else state.toErrorOutput("ProposalAndCommitAndValidConInput when in DecidedStep")

        | ProposalAndPolkaAndInvalidConInput =>
            // TODO
            state.toNoConsensusOutput()

        // line 55
        | RoundSkipConInput(r) => 
            if (r > state.round)
                state.toSkipRoundOutput(r)
            else state.toErrorOutput("RoundSkipConInput for round not bigger than current round")

        // line 57
        | TimeoutProposeConInput(height_round) => 
            val h = height_round._1
            val r = height_round._2
            if (h == state.height and r == state.round and state.step == ProposeStep)
                state
                    .with("step", PrevoteStep)
                    .toPrevoteOutput(state.p, state.height, state.round, Nil)
            else state.toErrorOutput("TimeoutProposeConInput")

        // line 61
        | TimeoutPrevoteConInput(height_round) => 
            val h = height_round._1
            val r = height_round._2
            if (h == state.height and r == state.round and state.step == PrevoteStep)
                // TODO: should we send precommit nil again ?
                state
                    .with("step", PrecommitStep)
                    .toPrecommitOutput(state.p, state.height, state.round, Nil)
            else state.toErrorOutput("TimeoutPrevoteConInput")

        // line 65
        | TimeoutPrecommitConInput(height_round) => 
            val h = height_round._1
            val r = height_round._2
            if (h == state.height and r == state.round)
                // TODO: here we should call newRound. For this we would need to know whether
                // we are proposer for next round.
                state.toSkipRoundOutput(state.round + 1)
            else state.toErrorOutput("TimeoutPrecommitConInput")

        }

}
