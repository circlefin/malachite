// -*- mode: Bluespec; -*-

/*
  TODO: round switch upon f+1 messages from the future is not done yet.
  We need to catch the event from the bookkeeper
*/ 

module driver {

import types.* from "./types"
import consensus.* from "./consensus"
import voteBookkeeper.* from "./voteBookkeeper"

type DriverState = {
    bk : Bookkeeper,
    cs : ConsensusState,
    proposals: Set[Proposal_t],
    valset: Address_t -> int,
    executedInputs: List[(ConsensusInput, Height_t, Round_t, Step_t)], // We record that to have the information in the trace
    pendingInputs: Set[(ConsensusInput, Height_t, Round_t)],
    pendingStepChange: Step_t, // "" if last consensus call did not change the step
    started: bool,
    voteKeeperOutput: VoteKeeperOutput, //debug TODO
    chain : List[Value_t],
    nextValueToPropose: Value_t,
}

pure def initDriver  (v: Address_t, vs: Address_t -> int) : DriverState = {
    val tvp = vs.keys().fold(0, (sum, key) => sum + vs.get(key))
    {
    bk: initBookKeeper(0, tvp),
    cs: initConsensusState(v),
    proposals: Set(),
    valset: vs,
    executedInputs: List(),
    pendingInputs: Set(),
    pendingStepChange: "",
    started: false,
    voteKeeperOutput: NoVKOutput, // debug
    chain : List(),
    nextValueToPropose: "", 
    }
}

type NodeState = {
    es: DriverState,
    timeout: Set[(Timeout_t, Height_t, Round_t)],
    incomingVotes: Set[Vote_t],
    incomingProposals: Set[Proposal_t],
}

pure def initNode  (v: Address_t, vs: Address_t -> int) : NodeState = {
    es: initDriver(v,vs),
    timeout: Set(),
    incomingVotes: Set(),
    incomingProposals: Set(),
}

type DriverInput = {
    name: str, // TODO: make a set of values. 
    proposal: Proposal_t,
    vote: Vote_t,
    timeout: str,
    csInput: ConsensusInput,
    nextValueToPropose: Value_t,
    step: Step_t
}

val defaultInput: DriverInput = {
    name: "",
    proposal: { src: "", height: 0, round: 0, proposal: "", validRound: 0 },
    vote: { src: "", height: 0, round: 0, step: "", id: "", },
    timeout: "",
    csInput: defaultConsensusInput,
    nextValueToPropose: "",
    step: ""
}

// this is to match the type from the bookkeeper. When we add heights there we should
// unify
pure def toVote(v: Vote_t): Vote = 
    val voteType = if (v.step == "Precommit") Precommit else Prevote
    val value = if (v.id == "nil") Nil else Val(v.id)
    { voteType: voteType, height: v.height, round: v.round, valueId: value, srcAddress: v.src }

val defaultConsensusInput : ConsensusInput = { name : "", height : 0, round: 0, value: "", vr: 0 }    



val emptyProposal : Proposal_t=
    { src: "none", height: 0, round: 0, proposal: "none", validRound: 0 }

val emptyVote = 
    { src: "none", height: 0, round: 0, step: "None", id: "None" }

//
// Interface to app and/or mempool (Proposer, getValue, valid)
// 

// In the implementation this could be a callback to the application. But it needs to be
// a function, that is, any two validators need to agree on this
pure def Proposer(valset: Address_t -> int, height: Height_t, round: Round_t) : Address_t = {
    // Here: rotating coordinator. We can do something more clever actually using the valset
    val prop = (round + 1) % 4
    if (prop == 0) "v1"
    else if  (prop == 1) "v2"
    else if  (prop == 2) "v3"
    else "v4"
}

pure def getValue(es: DriverState) : Value_t =  es.nextValueToPropose

pure def valid(p: Proposal_t) :bool = {
    // for simulation, if the value is "invalid", so is the proposal
    // if this is to become "non-deterministic", we must push it
    // and its use into the state machine
    p.proposal != "invalid"
}

type ConsensusCall = {
    es: DriverState,
    csInput: ConsensusInput,
    out: ConsensusOutput
}

pure def ListContains(list, value) = 
    list.foldl(false, (s,x) => s or x == value)

// check whether the event has been already sent to consensus. If not, do so.
pure def callConsensus (es: DriverState, bk: Bookkeeper, csInput: ConsensusInput) : (DriverState, ConsensusOutput) = {
    // Check whether we already executed the event already
    // TODO: Perhaps this check should be more fine-grained. Perhaps some inputs should only be acted on once per
    //          round, independent of the step.
    // TODO: get rid of bk parameter
    if (es.executedInputs.ListContains((csInput, es.cs.height, es.cs.round, es.cs.step)))
        ({ ...es, bk: bk, cs: es.cs }, defaultOutput)
    else
        // Go to consensus
        val res = consensus(es.cs, csInput)
        // Record that we executed the event
        val csInputs = es.executedInputs.append((csInput, es.cs.height, es.cs.round, es.cs.step))

        ({ ...es, bk: bk, cs: res.cs, executedInputs: csInputs }, res.out)
}


// We do this if the driver receives a Precommit
pure def precommit(es: DriverState, input: DriverInput, vkOutput: VoteKeeperOutput): (DriverState, ConsensusOutput) =
    match vkOutput {
        | PrecommitValueVKOutput(round_value) =>
            val round = round_value._1
            val value = round_value._2
            if (es.proposals.exists(x => round == x.round and value == id(x.proposal)))
                callConsensus(es, es.bk, {  name : "ProposalAndCommitAndValid",
                                            height : input.vote.height,
                                            round: input.vote.round,
                                            value: input.vote.id,
                                            vr: -1})
            else if (round == es.cs.round)
                callConsensus(es, es.bk,  { name: "PrecommitAny",
                                            height: input.vote.height,
                                            round: input.vote.round,
                                            value: input.vote.id,
                                            vr: -1})   
            else if (round > es.cs.round)
                // if it is for a future round I can trigger skipround
                // TODO: should we really do this. It is dead code as the f+1 event already happened
                callConsensus(es, es.bk,  { name: "RoundSkip",
                                            height: input.vote.height,
                                            round: input.vote.round,
                                            value: input.vote.id,
                                            vr: -1})     
            // ignore messages from past rounds
            else (es, defaultOutput)
        | PrecommitAnyVKOutput(round) =>
            if (round == es.cs.round)
                callConsensus(es, es.bk,  { name: "PrecommitAny",
                                            height: input.vote.height,
                                            round: input.vote.round,
                                            value: input.vote.id,
                                            vr: -1})
            else (es, defaultOutput)

        | SkipVKOutput(round) => 
            if (round > es.cs.round)
                callConsensus(es, es.bk,  { name: "RoundSkip",
                                            height: input.vote.height,
                                            round: input.vote.round,
                                            value: input.vote.id,
                                            vr: -1}) 
                else (es, defaultOutput)

        // none of the supported Precommit events. Do nothing
        | _ => (es, defaultOutput)
    }

// We do this if the driver receives a Prevote
// TODO: input is only used for the Skip case. We could just use the round provided in
// SkipVKOutput(round), which is equal to input.vote.id. The height should probably also be 
// recorded in the VoteKeeper output.
pure def prevote(es: DriverState, input: DriverInput, vkOutput: VoteKeeperOutput): (DriverState, ConsensusOutput) =
    // TODO: events do not have heights now.
    // TODO: Polka implications missing.
    match vkOutput {
        | PolkaValueVKOutput(round_value) =>
            val round = round_value._1
            val value = round_value._2
            if (round < es.cs.round and
                es.proposals.exists(p => p.round == es.cs.round and
                round == p.validRound and id(p.proposal) == value)) 
                    callConsensus(es, es.bk, {  name: "ProposalAndPolkaPreviousAndValid",
                                                height: es.cs.height,
                                                round: es.cs.round,
                                                value: value,
                                                vr: round})   
                                                // TODO: the value should come from the proposal   
            else if (round == es.cs.round)
                if (es.proposals.exists(p => p.round == es.cs.round and
                    id(p.proposal) == value)) 
                    val pend = (  {  name: "ProposalAndPolkaAndValid",
                                                height: es.cs.height,
                                                round: es.cs.round,
                                                value: value,
                                                vr: round}, es.cs.height, es.cs.round)   
                    callConsensus( { ...es,  pendingInputs: es.pendingInputs.union(Set(pend)) },
                                    es.bk, 
                                    {   name: "PolkaAny",
                                        height: es.cs.height,
                                        round: es.cs.round,
                                        value: value,
                                        vr: round})   
                else
                    // there is no matching proposal
                    callConsensus(es, es.bk, {  name: "PolkaAny",
                                                height: es.cs.height,
                                                round: es.cs.round,
                                                value: value,
                                                vr: round})   

            // It is for a future round
            // TODO: we might check whether it is for a future round and jump
            else (es, defaultOutput)

        | PolkaAnyVKOutput(round) =>
            if (round == es.cs.round)
                    // call consensus and remember that we did it
                    callConsensus(es, es.bk, {  name: "PolkaAny",
                                                height: es.cs.height,
                                                round: es.cs.round,
                                                value: "???", // CHECK: what value to use here?
                                                vr: round})  
            // TODO: we might check whether it is for a future round and jump
            else (es, defaultOutput)
                
        | PolkaNilVKOutput(round) => 
            if (round == es.cs.round)
                callConsensus(es, es.bk,  { name: "PolkaNil",
                                            height: es.cs.height,
                                            round: es.cs.round,
                                            value: "nil",
                                            vr: round})
            else (es, defaultOutput)

        | SkipVKOutput(round) => 
            if (round > es.cs.round)
                callConsensus(es, es.bk,  { name: "RoundSkip",
                                            height: input.vote.height,
                                            round: input.vote.round,
                                            value: input.vote.id,
                                            vr: -1}) 
            else (es, defaultOutput)
        
        | _ => (es, defaultOutput)
    }

// We do this if a timeout expires at the driver
pure def timeout(es: DriverState, input: DriverInput): (DriverState, ConsensusOutput) = {
    // TODO: We assume that the timeout event is always for the current round. If this is not
    // the case, we need to encode it in the input to which round the timeout belongs
    val csInput: ConsensusInput = {name: input.timeout, 
                        height: es.cs.height,
                        round: es.cs.round, 
                        value: "", 
                        vr: 0}
    callConsensus(es, es.bk, csInput)
}

// TODO: change result type of proposal (an other similar functions to this so that consensus can be called at one point in the driver
// type MuxResult = {es: DriverState, ci: ConsensusInput}


// We do this if the driver receives a proposal
pure def proposalMsg(es: DriverState, input: DriverInput) : (DriverState, ConsensusOutput) = {
// TODO: result should be 

    val newES = { ...es, proposals: es.proposals.union(Set(input.proposal))}
    if (input.proposal.src != Proposer(es.valset, input.proposal.height, input.proposal.round))
        // proposer does not match the height/round of the proposal
        // keep ES (don't use newES here), that is, drop proposal
        (es, defaultOutput)
    else if (valid(input.proposal)) 
        val receivedCommit = checkThreshold(    newES.bk, 
                                                input.proposal.round, 
                                                Precommit, 
                                                ValueThreshold(id(input.proposal.proposal)))
        if (receivedCommit)
            // we have a commit that matches the proposal. We don't need to compare against
            // es.cs.round
            // TODO: check heights in bookkeeper
            callConsensus(  newES, 
                            newES.bk,  
                                {   name: "ProposalAndCommitAndValid",
                                    height: input.proposal.height,
                                    round: input.proposal.round,
                                    value: id(input.proposal.proposal),
                                    vr: input.proposal.validRound})  
        else if (input.proposal.round != es.cs.round or input.proposal.height != es.cs.height) 
            // the proposal is from the right proposer and valid, but not for this round
            // keep the proposal, do nothing else
            (newES, defaultOutput)
        else
            // for current round and q, valid, and from right proposer
            val receivedPolkaValidRoundVal = checkThreshold(newES.bk,   
                                                            input.proposal.validRound, 
                                                            Prevote, 
                                                            ValueThreshold(id(input.proposal.proposal)))
            val receivedPolkaCurrentVal = checkThreshold(   newES.bk, 
                                                            newES.cs.round, 
                                                            Prevote, 
                                                            ValueThreshold(id(input.proposal.proposal)))
            val receivedCommitCurrentVal = checkThreshold(  newES.bk, 
                                                            newES.cs.round, 
                                                            Precommit, 
                                                            ValueThreshold(id(input.proposal.proposal)))
            if (newES.cs.step == "propose") 
                if (input.proposal.validRound == -1)
                    if (receivedPolkaCurrentVal)
                        callConsensus(  newES, 
                                        newES.bk,  
                                        {   name: "ProposalAndPolkaAndValid",
                                            height: input.proposal.height,
                                            round: input.proposal.round,
                                            value: id(input.proposal.proposal),
                                            vr: input.proposal.validRound})   
                    else
                        callConsensus(  newES, 
                                        newES.bk,  
                                        {   name: "Proposal",
                                            height: input.proposal.height,
                                            round: input.proposal.round,
                                            value: id(input.proposal.proposal),
                                            vr: input.proposal.validRound})    
                else if (receivedPolkaValidRoundVal)
                    callConsensus(  newES, 
                                    newES.bk,  
                                    {   name: "ProposalAndPolkaPreviousAndValid",
                                        height: input.proposal.height,
                                        round: input.proposal.round,
                                        value: id(input.proposal.proposal),
                                        vr: input.proposal.validRound})  
                else  
                    (newES, defaultOutput)
            else if (newES.cs.step == "Prevote" or newES.cs.step == "Precommit") 
                if (receivedCommitCurrentVal)
                    // here we need to call both, Commit and Polka. 
                    // We do commit and append polka to pending
                    val pend = ( {  name: "ProposalAndPolkaAndValid",
                                    height: newES.cs.height,
                                    round: newES.cs.round,
                                    value: id(input.proposal.proposal),
                                    vr: input.proposal.validRound},
                                newES.cs.height,
                                newES.cs.round)
                    callConsensus(  { ...newES, pendingInputs: newES.pendingInputs.union(Set(pend))}, 
                                    newES.bk,  
                                    {   name: "ProposalAndCommitAndValid",
                                        height: newES.cs.height,
                                        round: newES.cs.round,
                                        value: id(input.proposal.proposal),
                                        vr: input.proposal.validRound})  
                else if (receivedPolkaCurrentVal)
                    callConsensus(  newES , 
                                    newES.bk, 
                                    {   name: "ProposalAndPolkaAndValid",
                                    height: newES.cs.height,
                                    round: newES.cs.round,
                                    value: id(input.proposal.proposal),
                                    vr: input.proposal.validRound})  
                else 
                    (newES, defaultOutput)
            else
                (newES, defaultOutput)
    else 
        // (not(valid(input.proposal))) 
        // keep ES (don't use newES here), that is, drop proposal
        if (es.cs.step == "propose" and es.cs.round == input.proposal.round and es.cs.height == input.proposal.height)
            if (checkThreshold(es.bk, es.cs.round, Prevote, ValueThreshold(id(input.proposal.proposal))))
                val csInput: ConsensusInput = {name: "ProposalAndPolkaAndInValid", 
                                    height: es.cs.height,
                                    round: es.cs.round, 
                                    value: id(input.proposal.proposal), 
                                    vr: input.proposal.validRound}
                callConsensus(es, es.bk, csInput)
            else 
                val csInput: ConsensusInput = {name: "ProposalInvalid", 
                                    height: es.cs.height,
                                    round: es.cs.round, 
                                    value: id(input.proposal.proposal), 
                                    vr: input.proposal.validRound}
                callConsensus(es, es.bk, csInput)
        else 
            (es, defaultOutput)
}

// We do this after calling consensus to remember whether the step has changed
pure def recordStepChange (old: DriverState, new: DriverState) : DriverState = {
    if (old.cs.step == new.cs.step)
        { ...new, pendingStepChange: "" }
    else
        { ...new, pendingStepChange: new.cs.step}
}

// We do this if a step change of the consensus state machine was recorded
pure def StepChange (es: DriverState) : (DriverState, ConsensusOutput) = {
    // TODO: First add to pending precommitany if it exists
    val newES = 
        if (checkThreshold(es.bk, es.cs.round, Precommit, AnyThreshold)) 
            val pend = (    {   name: "PrecommitAny",
                                height: es.cs.height,
                                round: es.cs.round,
                                value: "",
                                vr: -1},
                            es.cs.height,
                            es.cs.round) 
            { ...es, pendingInputs: es.pendingInputs.union(Set(pend))}
        else es

    // Then check proposal
    val proposer = Proposer(newES.valset, newES.cs.height, newES.cs.round)
    val propSet = newES.proposals.filter(x => x.src == proposer and x.height == newES.cs.height and x.round == newES.cs.round)
    if (propSet != Set())
        val proposal = propSet.fold(emptyProposal, (sum, y) => y) 
                    // TODO:    If the proposer is faulty there might be multiple
                    //          figure out what to do
        val input = { ...defaultInput, name: "proposal", proposal: proposal}
        proposalMsg(newES, input) 

    // then go into step distinction but only consider rules without proposals
    else if (es.pendingStepChange == "Prevote")
        // If we have PolkaNil we don't start the timeout
        if (checkThreshold(newES.bk, newES.cs.round, Prevote, NilThreshold)) 
            callConsensus(es, es.bk, {  name: "PolkaNil",
                                        height: newES.cs.height,
                                        round: newES.cs.round,
                                        value: "nil",
                                        vr: -1})  
        else if (checkThreshold(newES.bk, newES.cs.round, Prevote, AnyThreshold)) 
            callConsensus(es, es.bk, {  name: "PolkaAny",
                                        height: newES.cs.height,
                                        round: newES.cs.round,
                                        value: "",
                                        vr: -1})  
        else
            (es, defaultOutput)
    else
        // For steps "newRound", "propose", "Precommit", there are no specific rules to check
        (es, defaultOutput)
}



// We do this when we need to jump to a new round
pure def skip (es: DriverState, r: int) : (DriverState, ConsensusOutput) = {
    // line 15
    val prop =  if (es.cs.validValue != "nil") es.cs.validValue
                else getValue(es)
    if (Proposer(es.valset, es.cs.height, es.cs.round + 1) == es.cs.p)
        callConsensus(es, es.bk,  { name: "NewRoundProposer",
                                    height: es.cs.height,
                                    round: r,
                                    // what value?
                                    value: prop,
                                    vr: es.cs.validRound})  
    else 
        callConsensus(es, es.bk,  { name: "NewRound",
                                    height: es.cs.height,
                                    round: r,
                                    value: "",
                                    vr: es.cs.validRound})  
    // what do we do now in the new round? Shouldn't we look whether we can build an event.
    // TODO: compute pending events.
}


// We do this when we have decided
pure def decided (es: DriverState, res: ConsensusOutput) : (DriverState, ConsensusOutput) = {
    // here we call consensus to set a new height, that is, to initialize the state machine
    // and then we call skip to start round 0

/*  // The following can be used to get to the next height. For now this
    // function does nothing
    // If we choose to move getValue out of the driver logic into the environment (gossip)
    // then, we would not do this here, but expect the environment to create a (to be defined) 
    // DriverInput      
    val s1 = callConsensus(es, es.bk, {name : "NewHeight",
                                    height : es.cs.height + 1,
                                    round: -1,
                                    value: "",
                                    vr: es.cs.validRound})  
    skip (s1._1, 0) 
*/
    ({ ...es, chain: es.chain.append(res.decided) } , res)
}


// take input out of pending inputs and then call consensus with that input
// We do this when the driver is asked to work on pending events
pure def PendingInput (es: DriverState): (DriverState, ConsensusOutput) = {
    val ev = es.pendingInputs.fold((defaultConsensusInput, -1, -1), (sum, y) => y)
    val newState = { ...es, pendingInputs: es.pendingInputs.exclude(Set(ev))}
     if (ev._2 == es.cs.height and ev._3 == es.cs.round)
        callConsensus(newState, es.bk, ev._1)
    else
       (newState, defaultOutput)
}


pure def setValue(es: DriverState, value: Value_t) : (DriverState, ConsensusOutput) =
    ({ ...es, nextValueToPropose: value }, defaultOutput)



/* *********************************************************
 * Main entry point
 * ********************************************************/


// TODO: return ConsensusInput so that we know from outside what event was fired.
pure def driverLogic (es: DriverState, input: DriverInput) : (DriverState, ConsensusOutput) = {
    // TODO: shall we check whether the sender actually is in the validator set
    if (input.name == "proposal") {
        val res = proposalMsg(es, input)
        if (res._2.name == "decided")
            decided (res._1, res._2)
        else if (res._2.name == "skipRound")
            skip (res._1, res._2.skipRound)
        else
            res
    }
    else if (input.name == "votemessage" and input.vote.step == "Precommit") {
        val res = applyVote(es.bk, toVote(input.vote), es.valset.get(input.vote.src), es.cs.round)
        val newES = { ...es, bk: res.bookkeeper, voteKeeperOutput: res.output }
        // only a commit event can come here. 
        val cons_res = precommit(newES, input, res.output) 
        if (cons_res._2.name == "decided")
            decided (cons_res._1, cons_res._2)
        else if (cons_res._2.name == "skipRound")
            skip (cons_res._1, cons_res._2.skipRound)
        else
            cons_res
    }
    else if (input.name == "votemessage" and input.vote.step == "Prevote") {
        val res = applyVote(es.bk, toVote(input.vote), es.valset.get(input.vote.src), es.cs.round)
        val newES = { ...es, bk: res.bookkeeper, voteKeeperOutput: res.output}
        // only a commit event can come here. 
        val cons_res = prevote(newES, input, res.output) 
        if (cons_res._2.name == "decided")
            // TODO: dead branch. But we should put this after consensus call logic into a function
            decided (cons_res._1, cons_res._2)
        else if (cons_res._2.name == "skipRound")
            skip (cons_res._1, cons_res._2.skipRound)
        else
            cons_res
    }
    else if (input.name == "timeout") {
        val res = timeout(es, input)
        // result should be vote or skip
        if (res._2.name == "skipRound")
            skip (res._1, res._2.skipRound)
            // skip starts a new round. This may involve getValue. If we choose to move the getValue
            // logic out of the driver, we wouldn't call skip here but add a (to be defined)
            // DriverInput
        else
            res
    }
    else if (input.name == "start") {
        //
        val new = { ...es, started: true}
        skip (new, 0)
    }
    else if (input.name == "pending") {
        PendingInput(es)
    }
    else if (input.name == "StepChange") {
        StepChange(es)
    }
    else if (input.name == "SetNextProposedValue")
        setValue(es, input.nextValueToPropose)
    else
        (es, defaultOutput)
}

// To address step change
pure def driver (es: DriverState, input: DriverInput) : (DriverState, ConsensusOutput) = {
    val res = driverLogic(es, input)
    (recordStepChange(es, res._1), res._2)
}


// This is a simple function that figures out in what external events (messages,
// timeouts, etc.) the node should act.
// currently this is linked in via the state machine. But we can move it into
// the functional layer/
pure def nextAction (state: NodeState) : (NodeState, DriverInput) = {
    if (not(state.es.started))
        (state, { ...defaultInput, name: "start" })

    else if (state.es.pendingStepChange != "")
        (state, { ...defaultInput, name: "StepChange" })

    else if (state.es.pendingInputs != Set())
        // this might be cheating as we look into the "es"
        (state, { ...defaultInput, name: "pending" })
        // I DID IT. TODO: In the "starkBFT Spec" Google doc, it is written that pending events
        // should be executed before new messages, which would requir to push this
        // branch up.
    else if (state.incomingProposals != Set())
        // pick proposal, remove it from incoming
        // val prop = state.incomingProposals.chooseSome()
        val prop = state.incomingProposals.fold (emptyProposal, (sum, y) => y)
        val newstate = { ...state, incomingProposals: state.incomingProposals.exclude(Set(prop))}
        (newstate, { ...defaultInput, name: "proposal", proposal: prop})

    else if (state.incomingVotes != Set())
        // pick vote, remove it from incoming
        // val vote = state.incomingVotes.chooseSome()
        val vote = state.incomingVotes.fold(emptyVote, (sum, y) => y)
        val newstate = { ...state, incomingVotes: state.incomingVotes.exclude(Set(vote))}
        (newstate, { ...defaultInput, name: "votemessage", vote: vote})

    else if (state.timeout.exists(x => x._2 == state.es.cs.height and 
        x._3 == state.es.cs.round))
        // pick timeout, remove it from incoming
        val timeouts = state.timeout.filter(x => x._2 == state.es.cs.height and 
            x._3 == state.es.cs.round)
        // val timeout = timeouts.chooseSome()
        val someTimeout = timeouts.fold(("", 0, 0), (sum, y) => y)
        val newstate = { ...state, timeout: state.timeout.exclude(Set(someTimeout))}
        (newstate, { ...defaultInput, name: "timeout", timeout: someTimeout._1})
    else
        (state, defaultInput)
}

// This function can be used to control test runs better.
pure def nextActionCommand (state: NodeState, command: str) : (NodeState, DriverInput) = {
    if (command == "start" and not(state.es.started))
        (state, { ...defaultInput, name: "start" })

    else if (command == "pending" and state.es.pendingInputs != Set())
        // this might be cheating as we look into the "es"
        (state, { ...defaultInput, name: "pending" })

    else if (command == "proposal" and state.incomingProposals != Set())
        // pick proposal, remove it from incoming
        // val prop = state.incomingProposals.chooseSome()
        val prop = state.incomingProposals.fold (emptyProposal, (sum, y) => y)
        val newstate = { ...state, incomingProposals: state.incomingProposals.exclude(Set(prop))}
        (newstate, { ...defaultInput, name: "proposal", proposal: prop})

    else if (command == "vote" and state.incomingVotes != Set())
        // pick vote, remove it from incoming
        // val vote = state.incomingVotes.chooseSome()
        val vote = state.incomingVotes.fold(emptyVote, (sum, y) => y)
        val newstate = { ...state, incomingVotes: state.incomingVotes.exclude(Set(vote))}
        (newstate, { ...defaultInput, name: "votemessage", vote: vote})

    else if (command == "timeout" and state.timeout.exists(x => x._2 == state.es.cs.height and 
        x._3 == state.es.cs.round))
        // pick timeout, remove it from incoming
        val timeouts = state.timeout.filter(x => x._2 == state.es.cs.height and 
            x._3 == state.es.cs.round)
        // val timeout = timeouts.chooseSome()
        val someTimeout = timeouts.fold(("", 0, 0), (sum, y) => y)
        val newstate = { ...state, timeout: state.timeout.exclude(Set(someTimeout))}
        (newstate, { ...defaultInput, name: "timeout", timeout: someTimeout._1})

    else
        (state, defaultInput)
}

}
