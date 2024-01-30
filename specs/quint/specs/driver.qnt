// -*- mode: Bluespec; -*-


module driver {
    import extraSpells.* from "../spells/extra"
    import types.* from "./types"
    import consensus.* from "./consensus"
    import votekeeper.* from "./votekeeper"

    // *************************************************************************
    // State
    // *************************************************************************

    type DriverState = {
        bk: Bookkeeper,
        cs: ConsensusState,
        proposals: Set[Proposal],
        valset: Address -> Weight,
        executedInputs: List[(ConsensusInput, Height, Round, Step)], // We record that to have the information in the trace
        pendingInputs: Set[(ConsensusInput, Height, Round)],
        pendingStepChange: Step, // NoStep if last consensus call did not change the step
        started: bool,
        voteKeeperOutput: VoteKeeperOutput,
        chain : List[Value],
    }

    pure def initDriver(v: Address, vs: Address -> Weight): DriverState = {
        bk: initBookKeeper(0, vs.mapSumValues()),
        cs: initConsensusState(v, 0),
        proposals: Set(),
        valset: vs,
        executedInputs: List(),
        pendingInputs: Set(),
        pendingStepChange: NoStep,
        started: false,
        voteKeeperOutput: NoVKOutput,
        chain : List(),
    }

    pure def existsProposal(state, round, value) = 
        state.proposals.exists(p => p.round == round and p.proposal == value)

    pure def existsProposalValid(state, round, value, validRound) = 
        state.proposals.exists(p => p.round == round and p.proposal == value and p.validRound == validRound)

    pure def setValue(s: NodeState, value: NonNilValue): NodeState =
        { ...s, nextValueToPropose: Val(value) }

    // *************************************************************************
    // Input
    // *************************************************************************

    type DriverInput = 
        | NoDInput
        | ProposalDInput(Proposal)
        | VoteDInput(Vote)
        | TimeoutDInput(Timeout)
        | StartDInput
        | PendingDInput(ConsensusInput)
        | StepChangeDInput
        | ProposeValueDInput(NonNilValue)

    val emptyProposal = mkProposal(noAddress, 0, 0, "", 0)
    val emptyVote = mkVote(Prevote, noAddress, 0, 0, Nil)

    // *************************************************************************
    // Interface to app and/or mempool (Proposer, getValue, valid)
    // *************************************************************************

    // In the implementation this could be a callback to the application. But it needs to be
    // a function, that is, any two validators need to agree on this
    pure def proposer(valset: Address -> Weight, height: Height, round: Round): Address = {
        // Here: rotating coordinator. We can do something more clever actually using the valset
        val prop = (round + 1) % 4
        if (prop == 0) "v1"
        else if  (prop == 1) "v2"
        else if  (prop == 2) "v3"
        else "v4"
    }

    pure def getValue(state: NodeState): Value = state.nextValueToPropose

    type ConsensusCall = {
        es: DriverState,
        csInput: ConsensusInput,
        out: ConsensusOutput
    }

    pure def ListContains(list, value) = 
        list.foldl(false, (s,x) => s or x == value)

    // check whether the event has been already sent to consensus. If not, do so.
    pure def callConsensus(es: DriverState, bk: Bookkeeper, csInput: ConsensusInput): (DriverState, ConsensusOutput) =
        // Check whether we already executed the event already
        // Note: Perhaps this check should be more fine-grained. Perhaps some inputs should only be acted on once per
        //          round, independent of the step.
        // Potential improvement: get rid of bk parameter. Not done yet, as we might refactor to have a single call
        //                        to consensus from the driver
        if (es.executedInputs.ListContains((csInput, es.cs.height, es.cs.round, es.cs.step)))
            ({ ...es, bk: bk, cs: es.cs }, NoConsensusOutput)
        else
            // Go to consensus
            val res = consensus(es.cs, csInput)
            // Record that we executed the event
            val csInputs = es.executedInputs.append((csInput, res.cs.height, res.cs.round, es.cs.step))
            ({ ...es, bk: bk, cs: res.cs, executedInputs: csInputs }, res.out)

    // We do this if the driver receives a Precommit
    // Potential improvement: vote could be removed because the vote data is already in vkOutput.
    pure def handlePrecommit(es: DriverState, vote: Vote, vkOutput: VoteKeeperOutput): (DriverState, ConsensusOutput) =
        match vkOutput {
            | PrecommitValueVKOutput(round_value) =>
                val r: Round = round_value._1
                val v: NonNilValue = round_value._2
                if (es.existsProposal(r, v))
                    callConsensus(es, es.bk, ProposalAndCommitAndValidCInput(vote.valueId))
                else if (r == es.cs.round)
                    callConsensus(es, es.bk, PrecommitAnyCInput)
                else if (r > es.cs.round)
                    // if it is for a future round I can trigger skipround
                    // Potential improvement: This is dead code as the f+1 event already happened
                    callConsensus(es, es.bk, RoundSkipCInput(vote.round))
                // ignore messages from past rounds
                else (es, NoConsensusOutput)
            | PrecommitAnyVKOutput(r) =>
                if (r == es.cs.round)
                    callConsensus(es, es.bk, PrecommitAnyCInput)
                else (es, NoConsensusOutput)
            | SkipVKOutput(r) => 
                if (r > es.cs.round)
                    callConsensus(es, es.bk, RoundSkipCInput(vote.round)) 
                else (es, NoConsensusOutput)
            // none of the supported Precommit events. Do nothing
            | _ => (es, NoConsensusOutput)
        }

    // We do this if the driver receives a Prevote
    pure def handlePrevote(es: DriverState, vkOutput: VoteKeeperOutput): (DriverState, ConsensusOutput) =
        match vkOutput {
            | PolkaValueVKOutput(round_value) =>
                val r = round_value._1
                val v: NonNilValue = round_value._2
                if (r < es.cs.round and es.existsProposalValid(es.cs.round, v, r))
                    callConsensus(es, es.bk, ProposalAndPolkaPreviousAndValidCInput((Val(v), r)))
                else if (r == es.cs.round)
                    if (es.existsProposal(es.cs.round, v))
                        val pending = (ProposalAndPolkaAndValidCInput(Val(v)), es.cs.height, es.cs.round)
                        val newES = { ...es,  pendingInputs: es.pendingInputs.union(Set(pending)) }
                        callConsensus(newES, es.bk, PolkaAnyCInput)   
                    else
                        // there is no matching proposal
                        callConsensus(es, es.bk, PolkaAnyCInput)
                else (es, NoConsensusOutput)

            | PolkaAnyVKOutput(r) =>
                if (r == es.cs.round)
                    // call consensus and remember that we did it
                    callConsensus(es, es.bk, PolkaAnyCInput)  
                else (es, NoConsensusOutput)
                    
            | PolkaNilVKOutput(r) => 
                if (r == es.cs.round)
                    callConsensus(es, es.bk, PolkaNilCInput)
                else (es, NoConsensusOutput)

            | SkipVKOutput(r) => 
                if (r > es.cs.round)
                    callConsensus(es, es.bk, RoundSkipCInput(r))
                else (es, NoConsensusOutput)
            
            | _ => (es, NoConsensusOutput)
        }

    // We do this if a timeout expires at the driver
    // We assume that the timeout event is always for the current round.
    pure def handleTimeout(es: DriverState, t: Timeout): (DriverState, ConsensusOutput) =
        match t {
            | ProposeTimeout => callConsensus(es, es.bk, TimeoutProposeCInput((es.cs.height, es.cs.round)))
            | PrevoteTimeout => callConsensus(es, es.bk, TimeoutPrevoteCInput((es.cs.height, es.cs.round)))
            | PrecommitTimeout => callConsensus(es, es.bk, TimeoutPrecommitCInput((es.cs.height, es.cs.round)))
        }

    // We do this if the driver receives a proposal
    pure def handleProposal(es: DriverState, prop: Proposal): (DriverState, ConsensusOutput) =
        val th = ValueThreshold(prop.proposal)
        if (prop.srcAddress != proposer(es.valset, prop.height, prop.round))
            // proposer does not match the height/round of the proposal
            // keep ES (don't use newES here), that is, drop proposal
            (es, NoConsensusOutput)
        else if (prop.proposal.isValid())
            val newES: DriverState = { ...es, proposals: es.proposals.union(Set(prop))}
            val receivedCommit = checkThreshold(newES.bk, prop.round, Precommit, th)
            if (receivedCommit)
                // we have a commit that matches the proposal. We don't need to compare against
                // es.cs.round
                callConsensus(newES, newES.bk, ProposalAndCommitAndValidCInput(Val(prop.proposal)))
            else if (es.cs.round != prop.round or es.cs.height != prop.height) 
                // the proposal is from the right proposer and valid, but not for this round
                // keep the proposal, do nothing else
                (newES, NoConsensusOutput)
            else
                // for current round and q, valid, and from right proposer
                val receivedPolkaCurrentVal = checkThreshold(newES.bk, newES.cs.round, Prevote, th)
                val propId: ValueId = id(Val(prop.proposal))
                if (newES.cs.step == ProposeStep)
                    val receivedPolkaValidRoundVal = checkThreshold(newES.bk, prop.validRound, Prevote, th)
                    if (prop.validRound == -1)
                        if (receivedPolkaCurrentVal)
                            callConsensus(newES, newES.bk, ProposalAndPolkaAndValidCInput(propId))
                        else
                            callConsensus(newES, newES.bk, ProposalCInput((prop.round, propId)))
                    else if (receivedPolkaValidRoundVal)
                        callConsensus(newES, newES.bk, ProposalAndPolkaPreviousAndValidCInput((propId, prop.validRound)))
                    else  
                        (newES, NoConsensusOutput)
                else if (newES.cs.step == PrevoteStep or newES.cs.step == PrecommitStep)
                    val receivedCommitCurrentVal = checkThreshold(newES.bk, newES.cs.round, Precommit, th)
                    if (receivedCommitCurrentVal)
                        // here we need to call both, Commit and Polka. 
                        // We do commit and append polka to pending
                        val pending = (ProposalAndPolkaAndValidCInput(propId), newES.cs.height, newES.cs.round)
                        val newES2 = { ...newES, pendingInputs: newES.pendingInputs.union(Set(pending))}
                        callConsensus(newES2, newES.bk, ProposalAndCommitAndValidCInput(propId))
                    else if (receivedPolkaCurrentVal)
                        callConsensus(newES, newES.bk, ProposalAndPolkaAndValidCInput(propId))
                    else (newES, NoConsensusOutput)
                else (newES, NoConsensusOutput)
        else // not(isValid(proposal))
            // keep ES (don't use newES here), that is, drop proposal
            if (es.cs.step == ProposeStep and es.cs.round == prop.round and es.cs.height == prop.height)
                if (checkThreshold(es.bk, prop.validRound, Prevote, th))
                    callConsensus(es, es.bk, ProposalAndPolkaAndInvalidCInput(id(Val(prop.proposal))))
                else 
                    callConsensus(es, es.bk, ProposalInvalidCInput)
            else (es, NoConsensusOutput)

    // We do this after calling consensus to remember whether the step has changed
    pure def recordStepChange(old: DriverState, new: DriverState) : DriverState =
        if (old.cs.step == new.cs.step)
            { ...new, pendingStepChange: NoStep }
        else
            { ...new, pendingStepChange: new.cs.step}

    // We do this if a step change of the consensus state machine was recorded
    pure def handleStepChange(es: DriverState) : (DriverState, ConsensusOutput) =
        // First add to pending precommitany if it exists
        val newES = 
            if (checkThreshold(es.bk, es.cs.round, Precommit, AnyThreshold)) 
                val pend = (PrecommitAnyCInput, es.cs.height, es.cs.round) 
                { ...es, pendingInputs: es.pendingInputs.union(Set(pend))}
            else es

        // Then check proposal
        val proposer_ = proposer(newES.valset, newES.cs.height, newES.cs.round)
        val propSet = newES.proposals.filter(x => x.srcAddress == proposer_ and x.height == newES.cs.height and x.round == newES.cs.round)
        if (propSet != Set())
            val proposal: Proposal = propSet.fold(emptyProposal, (sum, y) => y) 
            // If the proposer is faulty there might be multiple proposals
            // here we just pick one of them
            // TODO: check whether there is a liveness issue with that. Added a question about it:
            //       https://github.com/informalsystems/malachite/issues/103#issuecomment-1888753234
            handleProposal(newES, proposal)

        // then go into step distinction but only consider rules without proposals
        else if (es.pendingStepChange == PrevoteStep)
            // If we have PolkaNil we don't start the timeout
            if (checkThreshold(newES.bk, newES.cs.round, Prevote, NilThreshold)) 
                callConsensus(es, es.bk, PolkaNilCInput)
            else if (checkThreshold(newES.bk, newES.cs.round, Prevote, AnyThreshold)) 
                callConsensus(es, es.bk, PolkaAnyCInput)
            else
                (es, NoConsensusOutput)
        
        // For steps "newRound", "propose", "Precommit", there are no specific rules to check
        else (es, NoConsensusOutput)

    // We do this when we need to jump to a new round
    pure def handleSkip(es: DriverState, r: Round): (DriverState, ConsensusOutput) =
        if (proposer(es.valset, es.cs.height, es.cs.round + 1) == es.cs.p) // line 14
            callConsensus(es, es.bk, NewRoundProposerCInput(r))
        else 
            callConsensus(es, es.bk, NewRoundCInput(r)) 

    // We do this when we have decided
    pure def handleDecided(es: DriverState, res: ConsensusOutput, value: Value): (DriverState, ConsensusOutput) =
        // here we call consensus to set a new height, that is, to initialize the state machine
        // and then we call skip to start round 0

    /*  // The following can be used to get to the next height. For now this
        // function does nothing
        // If we choose to move getValue out of the driver logic into the environment (gossip)
        // then, we would not do this here, but expect the environment to create a (to be defined) 
        // DriverInput      
        val s1 = callConsensus(s, s.bk, NewHeightCInput(s.cs.height + 1))  
        skip (s1._1, 0) 
    */
        ({ ...es, chain: es.chain.append(value) }, res)

    // take input out of pending inputs and then call consensus with that input
    // We do this when the driver is asked to work on pending events
    pure def handlePendingInput(es: DriverState): (DriverState, ConsensusOutput) =
        val input_height_round = es.pendingInputs.fold((NoConsensusInput, -1, -1), (sum, y) => y)
        val newState = { ...es, pendingInputs: es.pendingInputs.exclude(Set(input_height_round))}
        if (input_height_round._2 == es.cs.height and input_height_round._3 == es.cs.round)
            callConsensus(newState, es.bk, input_height_round._1)
        else
            (newState, NoConsensusOutput)

    // *************************************************************************
    // Main entry point
    // *************************************************************************

    // Potential improvement: return ConsensusInput so that we know from outside what event was fired.
    pure def driverLogic(state: DriverState, input: DriverInput): (DriverState, ConsensusOutput) =
        // TODO: We need to check whether the sender actually is in the validator set. Where should
        //       we put the check?
        match input {
            | StartDInput =>
                handleSkip({ ...state, started: true }, 0)
            | ProposalDInput(proposal) =>
                val res = handleProposal(state, proposal)
                val newState = res._1
                val conOut = res._2
                match res._2 {
                    | DecidedOutput(v) => handleDecided(newState, conOut, v)
                    | SkipRoundOutput(r) => handleSkip(newState, r)
                    | _ => (newState, conOut)
                }
            | ProposeValueDInput(v) =>
                callConsensus(state, state.bk, ProposeValueCInput(v))
            | VoteDInput(vote) => 
                match vote.voteType {
                    | Precommit =>
                        val res = applyVote(state.bk, vote, state.valset.get(vote.srcAddress), state.cs.round)
                        val newState = { ...state, bk: res.bookkeeper, voteKeeperOutput: res.output}
                        // only a commit event can come here. 
                        val cons_res = handlePrecommit(newState, vote, res.output) 
                        val newState2 = cons_res._1
                        val conOut = cons_res._2
                        match conOut {
                            | DecidedOutput(v) => handleDecided(newState2, conOut, v)
                            | SkipRoundOutput(r) => handleSkip(newState2, r)
                            | _ => (newState2, conOut)
                        }
                    | Prevote =>
                        val res = applyVote(state.bk, vote, state.valset.get(vote.srcAddress), state.cs.round)
                        val newState = { ...state, bk: res.bookkeeper, voteKeeperOutput: res.output}
                        // only a commit event can come here. 
                        val cons_res = handlePrevote(newState, res.output) 
                        val newState2 = cons_res._1
                        val conOut = cons_res._2
                        match cons_res._2 {
                            // dead branch. But we should put this after consensus call logic into a function
                            | DecidedOutput(v) => handleDecided(newState2, conOut, v)
                            | SkipRoundOutput(r) => handleSkip(newState2, r)
                            | _ => (newState2, conOut)
                        }
                }
            | TimeoutDInput(timeout) =>
                val res = state.handleTimeout(timeout)
                val newState = res._1
                val conOut = res._2
                // result should be vote or skip
                match conOut {
                    | SkipRoundOutput(round) =>
                        handleSkip(newState, round)
                        // skip starts a new round. This may involve getValue. If we choose to move the getValue
                        // logic out of the driver, we wouldn't call skip here but add a (to be defined)
                        // DriverInput
                    | _ => (newState, conOut)
                }
            | PendingDInput(_) => // CHECK: why input is not used?
                handlePendingInput(state)
            | StepChangeDInput =>
                handleStepChange(state)
            | _ =>
                (state, NoConsensusOutput)
        }

    // To address step change
    pure def driver (state: DriverState, input: DriverInput): (DriverState, ConsensusOutput) =
        val res = driverLogic(state, input)
        (recordStepChange(state, res._1), res._2)

    // *************************************************************************
    // Node State
    // *************************************************************************

    type NodeState = {
        es: DriverState,
        timeouts: Set[(Timeout, Height, Round)],
        incomingVotes: Set[Vote],
        incomingProposals: Set[Proposal],
        getValueRequests: Set[(Height, Round)],
        nextValueToPropose: Value, 
    }

    pure def initNode(v: Address, vs: Address -> Weight): NodeState = {
        es: initDriver(v,vs),
        timeouts: Set(),
        incomingVotes: Set(),
        incomingProposals: Set(),
        getValueRequests: Set(),
        nextValueToPropose: Nil, 
    }

    pure def existsTimeout(state: NodeState): bool =
        state.timeouts.exists(x => x._2 == state.es.cs.height and x._3 == state.es.cs.round)

    // Pick an outstanding timeout and remove it from the incoming list.
    // Assumes that state.timeouts is not empty for current height and round, that is, that
    // existsTimeout has been called.
    pure def pickTimeout(state: NodeState): (NodeState, DriverInput) =
        val timeouts = state.timeouts.filter(x => x._2 == state.es.cs.height and x._3 == state.es.cs.round)
        // Potential improvement: We could check that timeouts is not empty
        // val someTimeout = timeouts.chooseSome()
        val someTimeout: (Timeout, Height, Round) = timeouts.fold((ProposeTimeout, 0, 0), (sum, y) => y)
        val newstate = { ...state, timeouts: state.timeouts.exclude(Set(someTimeout))}
        (newstate, TimeoutDInput(someTimeout._1))

    // This is a simple function that figures out in what external events (messages,
    // timeouts, etc.) the node should act.
    // currently this is linked in via the state machine. But we can move it into
    // the functional layer
    pure def nextAction(state: NodeState): (NodeState, DriverInput) =
        if (not(state.es.started))
            (state, StartDInput)

        // This is hard coded right now. I remove the request when served
        // also this prevents the StepChangeDInput for the proposer of a new round
        else if (state.getValueRequests.contains((state.es.cs.height, state.es.cs.round)))
            val newstate = { ...state, getValueRequests: state.getValueRequests.exclude(Set((state.es.cs.height, state.es.cs.round)))}
            if (state.nextValueToPropose != Nil)
                (newstate, ProposeValueDInput(getVal(state.nextValueToPropose)))
            else
                (newstate, ProposeValueDInput("nothing to propose"))

        else if (state.es.pendingStepChange != NoStep)
            (state, StepChangeDInput)

        else if (state.es.pendingInputs != Set())
            val input_height_round = state.es.pendingInputs.fold((NoConsensusInput, -1, -1), (sum, y) => y)
            // this might be cheating as we look into the "es"
            (state, PendingDInput(input_height_round._1))

        else if (state.incomingProposals != Set())
            // pick proposal, remove it from incoming
            // val prop = state.incomingProposals.chooseSome()
            val prop = state.incomingProposals.fold (emptyProposal, (sum, y) => y)
            val newstate = { ...state, incomingProposals: state.incomingProposals.exclude(Set(prop))}
            (newstate, ProposalDInput(prop))

        else if (state.incomingVotes != Set())
            // pick vote, remove it from incoming
            // val vote = state.incomingVotes.chooseSome()
            val vote = state.incomingVotes.fold(emptyVote, (sum, y) => y)
            val newstate = { ...state, incomingVotes: state.incomingVotes.exclude(Set(vote))}
            (newstate, VoteDInput(vote))

        else if (state.existsTimeout())
            state.pickTimeout()

        else
            (state, NoDInput)

    type Command =
        | StartCmd
        | PendingCmd
        | ProposeValueCmd
        | ProposalCmd
        | VoteCmd
        | TimeoutCmd

    // This function can be used to control test runs better.
    pure def nextActionCommand(state: NodeState, command: Command): (NodeState, DriverInput) =
        if (command == StartCmd and not(state.es.started))
            (state, StartDInput)

        else if (command == ProposeValueCmd and state.getValueRequests.contains((state.es.cs.height, state.es.cs.round)))
            val newstate = { ...state, getValueRequests: state.getValueRequests.exclude(Set((state.es.cs.height, state.es.cs.round)))}
            if (state.nextValueToPropose != Nil)
                (newstate, ProposeValueDInput(getVal(state.nextValueToPropose)))
            else
                (newstate, ProposeValueDInput("nothing to propose"))

        else if (command == PendingCmd and state.es.pendingInputs != Set())
            val input_height_round = state.es.pendingInputs.fold((NoConsensusInput, -1, -1), (sum, y) => y)
            // this might be cheating as we look into the "es"
            (state, PendingDInput(input_height_round._1))

        else if (command == ProposalCmd and state.incomingProposals != Set())
            // pick proposal, remove it from incoming
            // val prop = state.incomingProposals.chooseSome()
            val prop = state.incomingProposals.fold (emptyProposal, (sum, y) => y)
            val newstate = { ...state, incomingProposals: state.incomingProposals.exclude(Set(prop))}
            (newstate, ProposalDInput(prop))

        else if (command == VoteCmd and state.incomingVotes != Set())
            // pick vote, remove it from incoming
            // val vote = state.incomingVotes.chooseSome()
            val vote = state.incomingVotes.fold(emptyVote, (sum, y) => y)
            val newstate = { ...state, incomingVotes: state.incomingVotes.exclude(Set(vote))}
            (newstate, VoteDInput(vote))

        else if (command == TimeoutCmd and state.existsTimeout())
            state.pickTimeout()

        else
            (state, NoDInput)

}
