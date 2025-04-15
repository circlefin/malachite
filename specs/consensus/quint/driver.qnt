// -*- mode: Bluespec; -*-


module driver {
    import extraSpells.* from "./spells/extra"
    import basicSpells.* from "./spells/basic"
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
        chain: List[ValueStoreEntry],
        certificates: Set[Certificate], // we store Polkas and Commits we have seen
    }

    pure def initDriver(v: Address, vs: ValidatorSet, h: Height, chain: List[ValueStoreEntry]): DriverState = {
        bk: initBookKeeper(h, vs),
        cs: initConsensusState(v, h),
        proposals: Set(),
        valset: vs,
        executedInputs: List(),
        pendingInputs: Set(),
        pendingStepChange: NoStep,
        started: false,
        voteKeeperOutput: NoVKOutput,
        chain: chain,
        certificates: Set(),
    }

    pure def existsProposal(state, round, value) = 
        state.proposals.exists(p => p.round == round and p.proposal == value)

    pure def existsProposalValid(state, round, value, validRound) = 
        state.proposals.exists(p => p.round == round and p.proposal == value and p.validRound == validRound)

    pure def setValue(s: NodeState, value: NonNilValue): NodeState =
        { ...s, nextValueToPropose: Val(value) }



    // *************************************************************************
    // New Height Logic
    // *************************************************************************

    // *** helper functions ******************/

    pure def maxRound(S: Set[Round]): Round = 
        S.fold(0, (x,agg) => max(x,agg))

    pure def removeMessagesFromOlderHeights(ns: NodeState, height: Height) : NodeState =
        val proposals = ns.incomingProposals.filter(v => v.height >= height)
        val votes = ns.incomingVotes.filter(v => v.height >= height)
        val certificates = ns.incomingCertificates.filter(cert => cert.forall(v => v.height >= height))
        { ... ns,
            incomingProposals: proposals,
            incomingVotes: votes,
            incomingCertificates: certificates }



    // getNewHeightAction() manages the fast processing of messages when a new height h is
    // started. It is called from nextAction when a the consensus algorithm has not yet started,
    // and returns a node state with
    //    - an updated votekeeper with all votes of this height processed
    //    - all messages from the previous heights and the current height removed
    //    - pendingInputs updated with the returned ConsensusInput if there is one
    // and a driver input. 
    // It considers 3 cases:
    // 1) there is a proposal+commit certificate for that proposal in any round
    //    => a PendingDInput with ProposalAndCommitAndValidCInput is returned
    // 2) there is a VotekeeperOutput for round r > 0
    //    => a PendingDInput with NewRoundCInput is returned
    // 3) else 
    //    => only the Votekeeper is updated and StartDInput is returned

    pure def getNewHeightAction(ns: NodeState, vs: ValidatorSet, h: Height): (NodeState, DriverInput)  = 
        val proposalsForHeight = ns.incomingProposals.filter(v => v.height == h)
        val votesForHeight = ns.incomingVotes.filter(v => v.height == h)
        val maxVotesRound = maxRound(votesForHeight.map( vote => vote.round)) // needed to avoid skip rounds in applyVote
        val certificatesForHeight = ns.incomingCertificates.filter(cert => cert.forall(v => v.height == h))
        val maxCertRound = maxRound(certificatesForHeight.map( cert => maxRound(cert.map(vote => vote.round)))) // needed to avoid skip rounds in applyVote

        // init new driver with empty votekeeper
        val vkEmpty = initBookKeeper(h, vs)

        // load votekeeper with votes
        val vkWithVotes = votesForHeight.fold(vkEmpty, (vk, vote) => 
            val res = applyVote(vk, vote, maxVotesRound)
            res.bookkeeper          
        )    

        // load votekeeper with certificates
        val vkWithCertificates = certificatesForHeight.fold(vkWithVotes, (vk, cert) =>
            val res = applyCertificate(vk, cert, maxCertRound)
            res.bookkeeper
        )

        // check if there is a value to decide: decisionVal is a decision value or Nil
        val decisionValAndRound = vkWithCertificates.rounds.keys().fold((Nil, -1), (agg, rNr) =>
            if (agg._1 != Nil) agg
            else {
                vkWithCertificates.rounds.get(rNr).emittedOutputs.fold((Nil, -1), (agg2, output) =>
                    match output {
                        | PrecommitValueVKOutput(value) => 
                            val validProposals = proposalsForHeight.filter(p => p.proposal.isValid() and p.proposal == value._2 and p.srcAddress == proposer(vs,h,rNr))
                            if (validProposals != Set())
                                (Val(value._2),rNr)
                            else
                                agg2
                        | _ => agg2
                    }
                )
            }       
        )

        // else compute consensus round to start
        val roundToStart = vkWithCertificates.rounds.keys().fold(0, (agg, rNr) =>
            vkWithCertificates.rounds.get(rNr).emittedOutputs.fold(0, (agg2, output) =>
                match output {
                    | PolkaAnyVKOutput(round) => max(round,agg2)
                    | PolkaNilVKOutput(round) => max(round,agg2)
                    | PolkaValueVKOutput(output) => max(output._1,agg2)
                    | PrecommitAnyVKOutput(round) => max(round,agg2)
                    | PrecommitValueVKOutput(output) => max(output._1,agg2)
                    | SkipVKOutput(round) => max(round,agg2)
                    | _ => agg2
                }
            )
        )

        // determine consensus input, needed as parameter in PendingDInput return value and in pendingInputs
        val resultInput = if (decisionValAndRound._1 != Nil)
            val decisionVal : NonNilValue = getVal(decisionValAndRound._1)
            val decisionRound = decisionValAndRound._2
            Set((ProposalAndCommitAndValidCInput((decisionRound, decisionVal)),h, decisionRound))
        else if (roundToStart > 0)
            Set((NewRoundCInput(roundToStart), h, -1))
        else 
            Set()

        val resultDriver = { ...ns.es, bk: vkWithCertificates, pendingInputs: resultInput, started: true }

        val resultAction = if (decisionValAndRound._1 != Nil)
            val decisionVal : NonNilValue = getVal(decisionValAndRound._1)
            val decisionRound = decisionValAndRound._2
            PendingDInput(ProposalAndCommitAndValidCInput((decisionRound, decisionVal)))
        else if (roundToStart > 0) {
            if (isProposer({...ns, es:resultDriver}, h, roundToStart)) {
                PendingDInput(NewRoundProposerCInput(roundToStart))
            }
            else {
                PendingDInput(NewRoundCInput(roundToStart))
            }
        }
        else 
            StartDInput

        // remove messages from previous heights
        val ns2 = ns.removeMessagesFromOlderHeights(h)

        // result state new driver and processed votes and certificates removed
        // Note: processed proposals messages are not removed
        val resultNodeState = {...ns2,
            es: resultDriver,
            incomingVotes: ns2.incomingVotes.exclude(votesForHeight),
            incomingCertificates: ns2.incomingCertificates.exclude(certificatesForHeight) }

        (resultNodeState, resultAction)



    // *************************************************************************
    // Input / Output
    // *************************************************************************

    type DriverInput = 
        | NoDInput
        | ProposalDInput(Proposal)
        | VoteDInput(Vote)
        | CertificateDInput(Set[Vote])
        | TimeoutDInput(Timeout)
        | StartDInput
        | PendingDInput(ConsensusInput)
        | StepChangeDInput
        | ProposeValueDInput(NonNilValue)

    type DriverResult = {
        ds: DriverState,
        out: ConsensusOutput,
        certificate: Certificate,
    }

    pure def toDriverOutput(ds: DriverState, out: ConsensusOutput, cert: Certificate) : DriverResult =
        { ds: ds, out: out, certificate: cert }

    pure def toDriverOutputNoCertificate(ds: DriverState, out: ConsensusOutput) : DriverResult =
        { ds: ds, out: out, certificate: NoCertificate }


    // *************************************************************************
    // Interface to app and/or mempool (Proposer, getValue, valid)
    // *************************************************************************

    // In the implementation this could be a callback to the application. But it needs to be
    // a function, that is, any two validators need to agree on this
    pure def proposer(valset: Address -> Weight, height: Height, round: Round): Address = {
        // Here: rotating coordinator. We can do something more clever actually using the valset
        // Update: generalized to arbitrary validator set, same proposer at different heights for test cases
        val valList = valset.keys().fold(List(), (s, x) => s.append(x))
        val prop = (round + 1) % length(valList) // start with v2 as before
        valList[prop]
        // if (prop == 0) "v1"
        // else if  (prop == 1) "v2"
        // else if  (prop == 2) "v3"
        // else "v4"
    }

    pure def isProposer(state: NodeState, height: Height, round: Round) : bool = {
        state.es.cs.p == proposer(state.es.bk.validatorSet, height, round)
    }

    pure def getValue(state: NodeState): Value = state.nextValueToPropose

    type ConsensusCall = {
        es: DriverState,
        csInput: ConsensusInput,
        out: ConsensusOutput
    }

    pure def ListContains(list, value) = 
        list.foldl(false, (s,x) => s or x == value)


    pure def callConsensus(es: DriverState, bk: Bookkeeper, c: Certificate, csInput: ConsensusInput): DriverResult =
        // Check whether we already executed the event already
        // Note: Perhaps this check should be more fine-grained. Perhaps some inputs should only be acted on once per
        //          round, independent of the step.
        // Potential improvement: get rid of bk parameter. Not done yet, as we might refactor to have a single call
        //                        to consensus from the driver
        if (es.executedInputs.ListContains((csInput, es.cs.height, es.cs.round, es.cs.step)))
            toDriverOutput({ ...es, bk: bk, cs: es.cs }, NoConsensusOutput, c)
        else
            // Go to consensus
            val res = consensus(es.cs, csInput)
            // Record that we executed the event
            val csInputs = es.executedInputs.append((csInput, res.cs.height, res.cs.round, es.cs.step))
            toDriverOutput({ ...es, bk: bk, cs: res.cs, executedInputs: csInputs }, res.out, c)

    // We do this if the driver receives a Precommit
    // Potential improvement: vote could be removed because the vote data is already in vkOutput.
    pure def handlePrecommit(es: DriverState, vote: Vote, vkOutput: VoteKeeperOutput, c: Certificate): DriverResult =
        match vkOutput {
            | PrecommitValueVKOutput(round_value) =>
                val r: Round = round_value._1
                val v: NonNilValue = round_value._2
                if (es.existsProposal(r, v))
                    callConsensus(es, es.bk, c, ProposalAndCommitAndValidCInput(round_value))
                else if (r == es.cs.round)
                    callConsensus(es, es.bk, c, PrecommitAnyCInput)
                else if (r > es.cs.round)
                    // if it is for a future round I can trigger skipround
                    // Potential improvement: This is dead code as the f+1 event already happened
                    callConsensus(es, es.bk, c, RoundSkipCInput(vote.round))
                // ignore messages from past rounds
                else toDriverOutput(es, NoConsensusOutput, c)
            | PrecommitAnyVKOutput(r) =>
                if (r == es.cs.round)
                    callConsensus(es, es.bk, c, PrecommitAnyCInput)
                else toDriverOutputNoCertificate(es, NoConsensusOutput)
            | SkipVKOutput(r) => 
                if (r > es.cs.round)
                    callConsensus(es, es.bk, c, RoundSkipCInput(vote.round)) 
                else toDriverOutputNoCertificate(es, NoConsensusOutput)
            // none of the supported Precommit events. Do nothing
            | _ => toDriverOutputNoCertificate(es, NoConsensusOutput)
        }

    // We do this if the driver receives a Prevote
    pure def handlePrevote(es: DriverState, vkOutput: VoteKeeperOutput, c: Certificate): DriverResult =
        match vkOutput {
            | PolkaValueVKOutput(round_value) =>
                val r = round_value._1
                val v: NonNilValue = round_value._2
                if (r < es.cs.round and es.existsProposalValid(es.cs.round, v, r))
                    callConsensus(es, es.bk, c, ProposalAndPolkaPreviousAndValidCInput((Val(v), r)))
                else if (r == es.cs.round)
                    if (es.existsProposal(es.cs.round, v))
                        val pending = (ProposalAndPolkaAndValidCInput(Val(v)), es.cs.height, es.cs.round)
                        val newES = { ...es,  pendingInputs: es.pendingInputs.union(Set(pending)) }
                        callConsensus(newES, es.bk, c, PolkaAnyCInput)   
                    else
                        // there is no matching proposal
                        callConsensus(es, es.bk, c, PolkaAnyCInput)
                else toDriverOutput(es, NoConsensusOutput, c)

            | PolkaAnyVKOutput(r) =>
                if (r == es.cs.round)
                    // call consensus and remember that we did it
                    callConsensus(es, es.bk, c, PolkaAnyCInput)  
                else toDriverOutputNoCertificate(es, NoConsensusOutput)
                    
            | PolkaNilVKOutput(r) => 
                if (r == es.cs.round)
                    callConsensus(es, es.bk, c, PolkaNilCInput)
                else toDriverOutputNoCertificate(es, NoConsensusOutput)

            | SkipVKOutput(r) => 
                if (r > es.cs.round)
                    callConsensus(es, es.bk, c, RoundSkipCInput(r))
                else toDriverOutputNoCertificate(es, NoConsensusOutput)
            
            | _ => toDriverOutputNoCertificate(es, NoConsensusOutput)
        }

    // We do this if a timeout expires at the driver
    // We assume that the timeout event is always for the current round.
    pure def handleTimeout(es: DriverState, t: Timeout): DriverResult =
        match t {
            | ProposeTimeout => callConsensus(es, es.bk, NoCertificate, TimeoutProposeCInput((es.cs.height, es.cs.round)))
            | PrevoteTimeout => callConsensus(es, es.bk, NoCertificate, TimeoutPrevoteCInput((es.cs.height, es.cs.round)))
            | PrecommitTimeout => callConsensus(es, es.bk, NoCertificate, TimeoutPrecommitCInput((es.cs.height, es.cs.round)))
        }

    // We do this if the driver receives a proposal
    pure def handleProposal(es: DriverState, prop: Proposal): DriverResult =
        val th = ValueThreshold(prop.proposal)
        if (prop.srcAddress != proposer(es.valset, prop.height, prop.round))
            // proposer does not match the height/round of the proposal
            // keep ES (don't use newES here), that is, drop proposal
            toDriverOutputNoCertificate(es, NoConsensusOutput)
        else if (prop.proposal.isValid())
            val newES: DriverState = { ...es, proposals: es.proposals.union(Set(prop))}
            val receivedCommit = checkThreshold(newES.bk, prop.round, Precommit, th)
            if (receivedCommit)
                // we have a commit that matches the proposal. We don't need to compare against
                // es.cs.round
                callConsensus(newES, newES.bk, NoCertificate, ProposalAndCommitAndValidCInput((prop.round, prop.proposal))) 
            else if (es.cs.round != prop.round or es.cs.height != prop.height) 
                // the proposal is from the right proposer and valid, but not for this round
                // keep the proposal, do nothing else
                toDriverOutputNoCertificate(newES, NoConsensusOutput)
            else
                // for current round and q, valid, and from right proposer
                val receivedPolkaCurrentVal = checkThreshold(newES.bk, newES.cs.round, Prevote, th)
                val propId: ValueId = id(Val(prop.proposal))
                if (newES.cs.step == ProposeStep)
                    val receivedPolkaValidRoundVal = checkThreshold(newES.bk, prop.validRound, Prevote, th)
                    if (prop.validRound == -1)
                    // removed (see Issue #670)
//                        if (receivedPolkaCurrentVal)
//                            callConsensus(newES, newES.bk, NoCertificate, ProposalAndPolkaAndValidCInput(propId))
//                        else
                            callConsensus(newES, newES.bk, NoCertificate, ProposalCInput((prop.round, propId)))
                    else if (receivedPolkaValidRoundVal)
                        callConsensus(newES, newES.bk, NoCertificate, ProposalAndPolkaPreviousAndValidCInput((propId, prop.validRound)))
                    else  
                        toDriverOutputNoCertificate(newES, NoConsensusOutput)
                else if (newES.cs.step == PrevoteStep or newES.cs.step == PrecommitStep)
                    val receivedCommitCurrentVal = checkThreshold(newES.bk, newES.cs.round, Precommit, th)
                    if (receivedCommitCurrentVal) // FIXME: is this even reachable? receivedCommit is true here, right?
                        // here we need to call both, Commit and Polka. 
                        // We do commit and append polka to pending
                        val pending = (ProposalAndPolkaAndValidCInput(propId), newES.cs.height, newES.cs.round)
                        val newES2 = { ...newES, pendingInputs: newES.pendingInputs.union(Set(pending))}
                        callConsensus(newES2, newES.bk, NoCertificate, ProposalAndCommitAndValidCInput((prop.round, prop.proposal)))
                    else if (receivedPolkaCurrentVal)
                        callConsensus(newES, newES.bk, NoCertificate, ProposalAndPolkaAndValidCInput(propId))
                    else toDriverOutputNoCertificate(newES, NoConsensusOutput)
                else toDriverOutputNoCertificate(newES, NoConsensusOutput)
        else // not(isValid(proposal))
            // keep ES (don't use newES here), that is, drop proposal
            if (es.cs.step == ProposeStep and es.cs.round == prop.round and es.cs.height == prop.height)
                if (checkThreshold(es.bk, prop.validRound, Prevote, th))
                    callConsensus(es, es.bk, NoCertificate, ProposalAndPolkaAndInvalidCInput(id(Val(prop.proposal))))
                else 
                    callConsensus(es, es.bk, NoCertificate, ProposalInvalidCInput)
            else toDriverOutputNoCertificate(es, NoConsensusOutput)

    // We do this after calling consensus to remember whether the step has changed
    pure def recordStepChange(old: DriverState, new: DriverState) : DriverState =
        if (old.cs.step == new.cs.step)
            { ...new, pendingStepChange: NoStep }
        else
            { ...new, pendingStepChange: new.cs.step}

    // We do this if a step change of the consensus state machine was recorded
    pure def handleStepChange(es: DriverState) : DriverResult =
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
                callConsensus(es, es.bk, NoCertificate, PolkaNilCInput)
            else if (checkThreshold(newES.bk, newES.cs.round, Prevote, AnyThreshold)) 
                callConsensus(es, es.bk, NoCertificate, PolkaAnyCInput)
            else
                toDriverOutputNoCertificate(es, NoConsensusOutput)
        
        // For steps "newRound", "propose", "Precommit", there are no specific rules to check
        else toDriverOutputNoCertificate(es, NoConsensusOutput)

    // We do this when we need to jump to a new round
    pure def handleSkip(es: DriverState, r: Round): DriverResult =
        if (proposer(es.valset, es.cs.height, es.cs.round + 1) == es.cs.p) // line 14
            callConsensus(es, es.bk, NoCertificate, NewRoundProposerCInput(r))
        else 
            callConsensus(es, es.bk, NoCertificate, NewRoundCInput(r)) 

    // We do this when we have decided
    pure def handleDecided(es: DriverState, res: ConsensusOutput, rv: (Round, NonNilValue), c: Certificate): DriverResult =
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
        val commit = match es.bk.getCertificate(PrecommitValueVKOutput(rv)) {
            | Commit(v) => v
            | _ => Set() // This cannot happen
        }
        val v = rv._2
        val prop = es.proposals.filter(p => and {   p.height == es.cs.height,
                                                    p.round == rv._1,
                                                    p.proposal == rv._2}
        ).fold(emptyProposal, (acc, i) => i) //chooseSome(),
        val dc = {decision: prop, commit: commit}
        // TODO: store proposal in chain instead of value
        toDriverOutput({ ...es, chain: es.chain.append(dc) }, res, c)

    // take input out of pending inputs and then call consensus with that input
    // We do this when the driver is asked to work on pending events
    pure def handlePendingInput(es: DriverState): DriverResult =
        val input_height_round = es.pendingInputs.fold((NoConsensusInput, -1, -1), (sum, y) => y)
        val newState = { ...es, pendingInputs: es.pendingInputs.exclude(Set(input_height_round))}
        if (input_height_round._2 == es.cs.height and input_height_round._3 == es.cs.round)
            callConsensus(newState, es.bk, NoCertificate, input_height_round._1)
        else
            toDriverOutputNoCertificate(newState, NoConsensusOutput)

    // *************************************************************************
    // Main entry point
    // *************************************************************************

    // Potential improvement: return ConsensusInput so that we know from outside what event was fired.
    pure def driverLogic(state: DriverState, input: DriverInput): DriverResult =
        // TODO: We need to check whether the sender actually is in the validator set. Where should
        //       we put the check?
        match input {
            | StartDInput =>
                handleSkip({ ...state, started: true }, 0)
            | ProposalDInput(proposal) =>
                val res = handleProposal(state, proposal)
                match res.out {
                    | DecidedOutput(v) => handleDecided(res.ds, res.out, v, NoCertificate)
                    | SkipRoundOutput(r) => handleSkip(res.ds, r)
                    | _ => toDriverOutputNoCertificate(res.ds, res.out)
                }
            | ProposeValueDInput(v) =>
                callConsensus(state, state.bk, NoCertificate, ProposeValueCInput(v))
            | VoteDInput(vote) => 
                val res = applyVote(state.bk, vote, state.cs.round)
                val cert = getCertificate(res.bookkeeper, res.output)
                val newState = { ...state, bk: res.bookkeeper, voteKeeperOutput: res.output}
                val cons_res = match vote.voteType {
                    | Precommit => handlePrecommit(newState, vote, res.output, cert) 
                    | Prevote => handlePrevote(newState, res.output, cert) 
                }
                match cons_res.out {
                    | DecidedOutput(v) => handleDecided(cons_res.ds, cons_res.out, v, cons_res.certificate)
                    | SkipRoundOutput(r) => handleSkip(cons_res.ds, r)
                    | _ => toDriverOutput(cons_res.ds, cons_res.out, cert) 
                }
            | CertificateDInput(votes) =>
                val res = applyCertificate(state.bk, votes, state.cs.round)
                val cert = getCertificate(res.bookkeeper, res.output)
                val newState = { ...state, bk: res.bookkeeper, voteKeeperOutput: res.output}
                // workaround for chooseSome
                val someVote = votes.fold(emptyVote, (sum, y) => y)
                val cons_res = match someVote.voteType {
                    | Precommit => handlePrecommit(newState, someVote, res.output, cert) 
                    | Prevote =>  handlePrevote(newState, res.output, cert) 
                }
                match cons_res.out {
                    | DecidedOutput(v) => handleDecided(cons_res.ds, cons_res.out, v, cons_res.certificate)
                    | SkipRoundOutput(r) => handleSkip(cons_res.ds, r)
                    | _ => toDriverOutput(cons_res.ds, cons_res.out, cert)
                }
            | TimeoutDInput(timeout) =>
                val res = state.handleTimeout(timeout)
                // result should be vote or skip
                match res.out {
                    | SkipRoundOutput(round) =>
                        handleSkip(res.ds, round)
                        // skip starts a new round. This may involve getValue. If we choose to move the getValue
                        // logic out of the driver, we wouldn't call skip here but add a (to be defined)
                        // DriverInput
                    | _ => toDriverOutputNoCertificate(res.ds, res.out)
                }
            | PendingDInput(_) => // CHECK: why input is not used?
                handlePendingInput(state)
            | StepChangeDInput =>
                handleStepChange(state)
            | _ =>
                toDriverOutputNoCertificate(state, NoConsensusOutput)
        }

    // To address step change
    pure def driver (state: DriverState, input: DriverInput): DriverResult =
        val res = driverLogic(state, input)
        toDriverOutput(recordStepChange(state, res.ds), res.out, res.certificate)

    // *************************************************************************
    // Node State
    // *************************************************************************

    type NodeState = {
        es: DriverState,
        timeouts: Set[(Timeout, Height, Round)],
        incomingVotes: Set[Vote],
        incomingProposals: Set[Proposal],
        incomingCertificates: Set[Set[Vote]],
        // buffers for sync protocol. We separate them from non-sync buffers to model that they have higher priority
        incomingSyncProposals: Set[Proposal],
        incomingSyncCertificates: Set[Set[Vote]],
        getValueRequests: Set[(Height, Round)],
        nextValueToPropose: Value, 
    }

    pure def initNode(v: Address, vs: Address -> Weight, height: Height): NodeState = {
        es: initDriver(v, vs, height, List()),
        timeouts: Set(),
        incomingVotes: Set(),
        incomingProposals: Set(),
        incomingCertificates: Set(),
        incomingSyncProposals: Set(),
        incomingSyncCertificates: Set(),
        getValueRequests: Set(),
        nextValueToPropose: Nil,
    }

    pure def existsTimeout(state: NodeState): bool =
        state.timeouts.exists(x => x._2 == state.es.cs.height and x._3 == state.es.cs.round)

    /// Pick an outstanding timeout and remove it from the incoming list.
    /// Assumes that state.timeouts is not empty for current height and round, that is, that
    /// existsTimeout has been called.
    pure def pickTimeout(state: NodeState): (NodeState, DriverInput) =
        val timeouts = state.timeouts.filter(x => x._2 == state.es.cs.height and x._3 == state.es.cs.round)
        // Potential improvement: We could check that timeouts is not empty
        // val someTimeout = timeouts.chooseSome()
        val someTimeout: (Timeout, Height, Round) = timeouts.fold((ProposeTimeout, 0, 0), (sum, y) => y)
        val newstate = { ...state, timeouts: state.timeouts.exclude(Set(someTimeout))}
        (newstate, TimeoutDInput(someTimeout._1))

    // This function figures out on what external events (messages), 
    // the node should act.
    // currently this is linked in via the state machine. But we can move it into
    // the functional layer
    pure def nextAction(state: NodeState): (NodeState, DriverInput) =
        val proposalsForCurrentHeight = state.incomingProposals.filter(p => p.height == state.es.cs.height)
        val votesForCurrentHeight = state.incomingVotes.filter(v => v.height == state.es.cs.height)
        // take only certificates with consistent height
        val certificatesForCurrentHeight = state.incomingCertificates.filter(cert => cert.forall(v => v.height == state.es.cs.height))
        val syncCertificatesForCurrentHeight = state.incomingSyncCertificates.filter(cert => cert.forall(v => v.height == state.es.cs.height))
        val syncProposalsForCurrentHeight = state.incomingSyncProposals.filter(p => p.height == state.es.cs.height)

        if (not(state.es.started))
            getNewHeightAction(state, state.es.valset, state.es.cs.height)

        else if (syncCertificatesForCurrentHeight != Set())
            // val cert = syncCertificatesForCurrentHeight.chooseSome()
            val cert = syncCertificatesForCurrentHeight.fold (Set(), (sum, y) => y)
            val newstate = { ...state, incomingSyncCertificates: state.incomingSyncCertificates.exclude(Set(cert))}
            (newstate, CertificateDInput(cert))

        else if (syncProposalsForCurrentHeight != Set())
            // pick proposal, remove it from incoming
            // val prop = syncProposalsForCurrentHeight.chooseSome()
            val prop = syncProposalsForCurrentHeight.fold (emptyProposal, (sum, y) => y)
            val newstate = { ...state, incomingSyncProposals: state.incomingSyncProposals.exclude(Set(prop))}
            (newstate, ProposalDInput(prop))

        // This is hard-coded right now. I remove the request when served
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

        else if (certificatesForCurrentHeight != Set())
            // val cert = certificatesForCurrentHeight.chooseSome()
            val cert = certificatesForCurrentHeight.fold (Set(), (sum, y) => y)
            val newstate = { ...state, incomingCertificates: state.incomingCertificates.exclude(Set(cert))}
            (newstate, CertificateDInput(cert))

        else if (proposalsForCurrentHeight != Set())
            // pick proposal, remove it from incoming
            // val prop = proposalsForCurrentHeight.chooseSome()
            val prop = proposalsForCurrentHeight.fold (emptyProposal, (sum, y) => y)
            val newstate = { ...state, incomingProposals: state.incomingProposals.exclude(Set(prop))}
            (newstate, ProposalDInput(prop))

        else if (votesForCurrentHeight != Set())
            // pick vote, remove it from incoming
            // val vote = votesForCurrentHeight.chooseSome()
            // TODO: only take votes for the current round.
            val vote = votesForCurrentHeight.fold(emptyVote, (sum, y) => y)
            val newstate = { ...state, incomingVotes: state.incomingVotes.exclude(Set(vote))}
            (newstate, VoteDInput(vote))

        else
            (state, NoDInput)

    type Command =
        | StartCmd
        | PendingCmd
        | ProposeValueCmd
        | ProposalCmd
        | VoteCmd
        | CertificateCmd
        | TimeoutCmd

    /// This function can be used to control test runs better.
    /// We can pass in what command should be executed, including timeouts.
    pure def nextActionCommand(state: NodeState, command: Command): (NodeState, DriverInput) =
        val proposalsForCurrentHeight = state.incomingProposals.filter(p => p.height == state.es.cs.height)
        val votesForCurrentHeight = state.incomingVotes.filter(v => v.height == state.es.cs.height)
        // take only certificates with consistent height
        val certificatesForCurrentHeight = state.incomingCertificates.filter(cert => cert.forall(v => v.height == state.es.cs.height))
        val syncCertificatesForCurrentHeight = state.incomingSyncCertificates.filter(cert => cert.forall(v => v.height == state.es.cs.height))
        val syncProposalsForCurrentHeight = state.incomingSyncProposals.filter(p => p.height == state.es.cs.height)
        if (command == StartCmd and not(state.es.started))
            getNewHeightAction(state, state.es.valset, state.es.cs.height)

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

        else if (command == ProposalCmd and proposalsForCurrentHeight != Set())
            // pick proposal, remove it from incoming
            // val prop = proposalsForCurrentHeight.chooseSome()
            val prop = proposalsForCurrentHeight.fold (emptyProposal, (sum, y) => y)
            val newstate = { ...state, incomingProposals: state.incomingProposals.exclude(Set(prop))}
            (newstate, ProposalDInput(prop))

        else if (command == VoteCmd and votesForCurrentHeight != Set())
            // pick vote, remove it from incoming
            // val vote = votesForCurrentHeight.chooseSome()
            val vote = votesForCurrentHeight.fold(emptyVote, (sum, y) => y)
            val newstate = { ...state, incomingVotes: state.incomingVotes.exclude(Set(vote))}
            (newstate, VoteDInput(vote))

        else if (command == CertificateCmd and certificatesForCurrentHeight != Set())
            // val cert = state.incomingCertificates.chooseSome()
            val cert = certificatesForCurrentHeight.fold (Set(), (sum, y) => y)
            val newstate = { ...state, incomingCertificates: state.incomingCertificates.exclude(Set(cert))}
            (newstate, CertificateDInput(cert))

        else if (command == TimeoutCmd and state.existsTimeout())
            state.pickTimeout()

        else
            (state, NoDInput)

}
