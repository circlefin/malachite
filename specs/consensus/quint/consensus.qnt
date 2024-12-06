// -*- mode: Bluespec; -*-

module consensus {

    import types.* from "./types"

    // *************************************************************************
    // State
    // *************************************************************************

    type ConsensusState = {
        p: Address,
        height: Height,
        round: Round,
        step: Step,
        lockedRound: Round,
        lockedValue: ValueId,
        validRound: Round,
        validValue: ValueId,
    }  

    pure def initConsensusState(p: Address, h: Height) : ConsensusState = {
        // The node address is only needed or setting the source of the Proposal and Vote messages.
        p: p,
        // round must be -1, as nothing should be done before a NewRound
        // function is called
        round: -1, 
        step: NoStep, 
        height: h,
        lockedRound: -1, 
        lockedValue: Nil, 
        validRound: -1, 
        validValue: Nil
    }

    // *************************************************************************
    // Input and output
    // *************************************************************************

    type ConsensusInput = 
        // For the initial state
        | NoConsensusInput
        // Start a new round, not as proposer.
        | NewRoundCInput(Round)
        // Start a new round as proposer.
        | NewRoundProposerCInput(Round)
        // GetValue returns (L18)
        | ProposeValueCInput(NonNilValue)
        // Receive a proposal without associated valid round.
        | ProposalCInput((Round, Value))
        // Receive a valid proposal with an associated valid round, attested by a a Polka(vr).
        | ProposalAndPolkaPreviousAndValidCInput((Value, Round))
        // Receive an invalid proposal: L26 and L32 when isValid(v) == false
        | ProposalInvalidCInput
        // Receive +2/3 prevotes for nil.
        | PolkaNilCInput
        // Receive +2/3 prevotes from different validators, not for the same value or nil.
        | PolkaAnyCInput
        // Proposal and 2/3+ prevotes for the proposal: L36 when valid and step >= prevote
        | ProposalAndPolkaAndValidCInput(Value)
        // found after Montebello
        // TODO: Discuss what to do about it
        | ProposalAndPolkaAndInvalidCInput(Value)
        // Receive +2/3 precommits from different validators, not for the same value or nil.
        | PrecommitAnyCInput
        // Proposal and 2/3+ commits for the proposal => decision
        | ProposalAndCommitAndValidCInput((Round, NonNilValue))
        // Receive +1/3 messages from different validators for a higher round.
        | RoundSkipCInput(Round) 
        // Timeout waiting for proposal.
        | TimeoutProposeCInput((Height, Round))
        // Timeout waiting for prevotes for a value.
        | TimeoutPrevoteCInput((Height, Round))
        // Timeout waiting for precommits for a value.
        | TimeoutPrecommitCInput((Height, Round))

    type ConsensusOutput =
        | NoConsensusOutput
        | ProposalOutput(Proposal)
        | GetValueAndScheduleTimeoutOutput((Height, Round, Timeout))
        | VoteOutput(Vote)
        | TimeoutOutput((Round, Timeout))
        | DecidedOutput((Round, NonNilValue))
        | SkipRoundOutput(Round)
        | ErrorOutput(str)

    pure def toNoConsensusOutput(state) = 
        { cs: state, out: NoConsensusOutput }
    pure def toProposalOutput(state, src, height, round, value, validRound) = 
        { cs: state, out: ProposalOutput(mkProposal(src, height, round, value, validRound)) }
    pure def toGetValueAndScheduleTimeoutOutput(state, height, round, timeout) =
        { cs: state, out: GetValueAndScheduleTimeoutOutput((height, round, timeout))}
    pure def toPrevoteOutput(state, src, height, round, valueId) = 
        { cs: state, out: VoteOutput(mkVote(Prevote, src, height, round, valueId)) }
    pure def toPrecommitOutput(state, src, height, round, valueId) = 
        { cs: state, out: VoteOutput(mkVote(Precommit, src, height, round, valueId)) }
    pure def toTimeoutOutput(state, timeout) = 
        { cs: state, out: TimeoutOutput((state.round, timeout)) }
    pure def toDecidedOutput(state, value, round) = 
        { cs: state, out: DecidedOutput((value, round)) }
    pure def toSkipRoundOutput(state, round) = 
        { cs: state, out: SkipRoundOutput(round) }
    pure def toErrorOutput(state, msg) = 
        { cs: state, out: ErrorOutput(msg) }

    // for testing
    pure def isVoteMsgWith(output, voteType, valueId) =
        match output {
            | VoteOutput(msg) => msg.voteType == voteType and msg.valueId == valueId
            | _ => false
        }

    // *************************************************************************
    // State transition
    // *************************************************************************

    type ConsensusResult = {
        cs: ConsensusState,
        out: ConsensusOutput,
    }

    pure def consensus(state: ConsensusState, input: ConsensusInput): ConsensusResult =
        match input {
        | NoConsensusInput => 
            state.toErrorOutput("NoConsensusInput is not a valid consensus input")
        
        // line 11.14
        | NewRoundProposerCInput(round) =>
            if (not(state.step.in(Set(UnstartedStep, NoStep))))
                state.toErrorOutput("state is not Unstarted or initial step")
            else if (round <= state.round)
                state.toErrorOutput("new round is not bigger than current round")
            else
                if (state.validValue == Nil) 
                    state
                        .with("step", ProposeStep)
                        .with("round", round)
                        .toGetValueAndScheduleTimeoutOutput(state.height, round, ProposeTimeout) // line 18
                else
                    val v: NonNilValue = getVal(state.validValue) // as validValue is non-nil in this branch
                    state
                        .with("step", ProposeStep)
                        .with("round", round)
                        .toProposalOutput(state.p, state.height, round, v, state.validRound) // line 19

        | ProposeValueCInput(value) =>
            // here we assume that the driver has checked that we are the proposer of 
            // the current round (and we have not sent proposal before)
            state.toProposalOutput(state.p, state.height, state.round, value, state.validRound) // line 19

        // line 11.20
        | NewRoundCInput(r) =>
            if (not(state.step.in(Set(UnstartedStep, NoStep))))
                state.toErrorOutput("state is not Unstarted or initial step")
            else if (r <= state.round)
                state.toErrorOutput("new round is not bigger than current round")
            else
                // We just report that a timeout should be started. The executor must take care
                // of figuring out whether it needs to record the round number and height per
                // timeout
                state
                    .with("step", ProposeStep)
                    .with("round", r)
                    .toTimeoutOutput(ProposeTimeout)                                  // line 21

        // line 22
        // Here it is assumed that 
        // - the value has been checked to be valid
        // - it is for the current round
        // The executor checks this upon receiving a propose message "ProposalMsg"
        | ProposalCInput(round_value) =>
            val r: Round = round_value._1
            val v: Value = round_value._2
            if (state.step == ProposeStep)
                if (state.lockedRound == -1 or state.lockedValue == v)                // line 23
                    state
                        .with("step", PrevoteStep)
                        .toPrevoteOutput(state.p, state.height, state.round, id(v))   // line 24
                else
                    state
                        .with("step", PrevoteStep)
                        .toPrevoteOutput(state.p, state.height, state.round, Nil)     // line 26
            else state.toErrorOutput("ProposalCInput when not in ProposeStep")

        // line 26
        | ProposalInvalidCInput =>
            if (state.step == ProposeStep) 
                state
                    .with("step", PrevoteStep)
                    .toPrevoteOutput(state.p, state.height, state.round, Nil)
            else state.toErrorOutput("ProposalCInput when not in ProposeStep")

        // line 28 (with valid(v) evaluates to true)
        | ProposalAndPolkaPreviousAndValidCInput(value_vr) =>
            val v: Value = value_vr._1
            val vr: Round = value_vr._2
            if (state.step == ProposeStep and vr >= 0 and vr < state.round)           // line 28
                if (state.lockedRound <= vr or state.lockedValue == v)                // line 29
                    state
                        .with("step", PrevoteStep)                                    // line 33
                        .toPrevoteOutput(state.p, state.height, state.round, id(v))   // line 30
                else 
                    state
                        .with("step", PrevoteStep)                                    // line 33
                        .toPrevoteOutput(state.p, state.height, state.round, Nil)     // line 32 (valid)
            else state.toErrorOutput("ProposalAndPolkaPreviousAndValidCInput")

        // line 28 (with valid(v) evaluates to false)
        | ProposalAndPolkaAndInvalidCInput =>
            state
                .with("step", PrevoteStep)                                    // line 33
                .toPrevoteOutput(state.p, state.height, state.round, Nil)     // line 32 (invalid)

        // line 34
        | PolkaAnyCInput =>
            if (state.step == PrevoteStep)
                // We just report that a timeout should be started. The executor must take care
                // of figuring out whether it needs to record the round number and height per
                // timeout
                state.toTimeoutOutput(PrevoteTimeout)
            else state.toErrorOutput("PolkaAnyCInput when not in PrevoteStep")

        // line 36
        | ProposalAndPolkaAndValidCInput(v) =>
            match state.step {
                | PrevoteStep =>
                    state
                        .with("step", PrecommitStep)                                  // line 41
                        .with("lockedValue", v)                                       // line 38
                        .with("lockedRound", state.round)                             // line 39
                        .with("validValue", v)                                        // line 42
                        .with("validRound", state.round)                              // line 43
                        .toPrecommitOutput(state.p, state.height, state.round, id(v)) // line 40
                | PrecommitStep =>
                    state
                        .with("validValue", v)                                         // line 42
                        .with("validRound", state.round)                               // line 43
                        .toNoConsensusOutput()
                | _ => state.toErrorOutput("ProposalAndPolkaAndValidCInput when not in PrevoteStep or PrecommitStep")
            }

        // line 44
        | PolkaNilCInput => 
            if (state.step == PrevoteStep)
                state
                    .with("step", PrecommitStep)
                    .toPrecommitOutput(state.p, state.height, state.round, Nil)
            else state.toErrorOutput("PolkaNilCInput when not in PrevoteStep")

        // line 47
        | PrecommitAnyCInput =>
            state.toTimeoutOutput(PrecommitTimeout)

        // line 49
        | ProposalAndCommitAndValidCInput(round_value) => 
                val r = round_value._1
                val v = round_value._2
            if (state.step != CommitStep)   // "CommitStep" doesn't exist in the arXiv paper. This ensures
                state                       // that we do not decide more than once in a round.
                    .with("step", CommitStep)
                    .toDecidedOutput(r, v)
            else state.toErrorOutput("ProposalAndCommitAndValidCInput when in CommitStep")

        // line 55
        | RoundSkipCInput(r) => 
            if (r > state.round)
                state
                    .with("step", UnstartedStep)
                    .toSkipRoundOutput(r)
            else state.toErrorOutput("RoundSkipCInput for round not bigger than current round")

        // line 57
        | TimeoutProposeCInput(height_round) => 
            val h = height_round._1
            val r = height_round._2
            if (h == state.height and r == state.round and state.step == ProposeStep)
                state
                    .with("step", PrevoteStep)
                    .toPrevoteOutput(state.p, state.height, state.round, Nil)
            else state.toErrorOutput("TimeoutProposeCInput")

        // line 61
        | TimeoutPrevoteCInput(height_round) => 
            val h = height_round._1
            val r = height_round._2
            if (h == state.height and r == state.round and state.step == PrevoteStep)
                // TODO: should we send precommit nil again ?
                state
                    .with("step", PrecommitStep)
                    .toPrecommitOutput(state.p, state.height, state.round, Nil)
            else state.toErrorOutput("TimeoutPrevoteCInput")

        // line 65
        | TimeoutPrecommitCInput(height_round) => 
            val h = height_round._1
            val r = height_round._2
            if (h == state.height and r == state.round)
                state
                    // CHECK: setting the step to UnstartedStep was added to match the code behavior
                    .with("step", UnstartedStep)
                    .toSkipRoundOutput(state.round + 1)
            else state.toErrorOutput("TimeoutPrecommitCInput")

        }

}
