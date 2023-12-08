// -*- mode: Bluespec; -*-

/*
  TODO: round switch upon f+1 messages from the future is not done yet.
  We need to catch the event from the bookkeeper
*/ 

module driver {

import extraSpells.* from "./extraSpells"
import types.* from "./types"
import consensus.* from "./consensus"
import voteBookkeeper.* from "./voteBookkeeper"

type DriverState = {
    bk: Bookkeeper,
    cs: ConsensusState,
    proposals: Set[Proposal],
    valset: Address -> int,
    executedInputs: List[(ConsensusInput, Height, Round)], // We record that to have the information in the trace
    pendingInputs: Set[(ConsensusInput, Height, Round)],
    started: bool,
    voteKeeperOutput: VoteKeeperOutput, //debug TODO
    chain : List[Value],
    nextValueToPropose: Value,
}

pure def initDriver(v: Address, vs: Address -> int): DriverState = {
    bk: initBookKeeper(0, vs.mapSumValues()),
    cs: initConsensusState(v, 0),
    proposals: Set(),
    valset: vs,
    executedInputs: List(),
    pendingInputs: Set(),
    started: false,
    voteKeeperOutput: NoVKOutput, // debug
    chain : List(),
    nextValueToPropose: Nil, 
}

pure def existsProposal(state, round, value) = 
    state.proposals.exists(p => p.round == round and p.proposal == value)

pure def existsProposalValid(state, round, value, validRound) = 
    state.proposals.exists(p => p.round == round and p.proposal == value and p.validRound == validRound)


type NodeState = {
    es: DriverState,
    timeout: Set[(Timeout, Height, Round)],
    incomingVotes: Set[Vote],
    incomingProposals: Set[Proposal],
}

pure def initNode  (v: Address, vs: Address -> int) : NodeState = {
    es: initDriver(v,vs),
    timeout: Set(),
    incomingVotes: Set(),
    incomingProposals: Set(),
}

type DriverInput = 
    | NoDInput
    | ProposalDInput(Proposal)
    | VoteDInput(Vote)
    | TimeoutDInput(Timeout)
    | StartDInput
    | PendingDInput(ConsensusInput)
    | SetNextProposedValueDInput(NonNilValue)

/*
encode the following decision tree

proposal
    invalid -> consensus event
    valid:  BK list of events 
            generate all Consensus events C
            feed all events to Consensus

Precommit
    BK applyVote
    (see whether we can make (A) 2f+1 Any for current or 
                             (B) 2f+1 plus proposal for current)
    -> feed all events to Consensus

Prevote
    BK applyVote 
    For current round PolkaVal -> PolkaAny and PolkaNil -> PolkaAny: list of Events
    check proposal: list of Consensus evens (considering line 28)
    -> feed all events to Consensus
*/

val emptyProposal = mkProposal(noAddress, 0, 0, "", 0)
val emptyVote = mkVote(Prevote, noAddress, 0, 0, Nil)

//
// Interface to app and/or mempool (Proposer, getValue, valid)
// 

// In the implementation this could be a callback to the application. But it needs to be
// a function, that is, any two validators need to agree on this
pure def Proposer(valset: Address -> int, height: Height, round: Round) : Address = {
    // Here: rotating coordinator. We can do something more clever actually using the valset
    val prop = (round + 1) % 4
    if (prop == 0) "v1"
    else if  (prop == 1) "v2"
    else if  (prop == 2) "v3"
    else "v4"
}

pure def getValue(es: DriverState) : Value =  es.nextValueToPropose

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
    if (es.executedInputs.ListContains((csInput, es.cs.height, es.cs.round)))
        ({ ...es, bk: bk, cs: es.cs }, NoConsensusOutput)
    else
        // Go to consensus
        val res = consensus(es.cs, csInput)
        // Record that we executed the event
        val csInputs = es.executedInputs.append((csInput, res.cs.height, res.cs.round))
        ({ ...es, bk: bk, cs: res.cs, executedInputs: csInputs }, res.out)

// We do this if the driver receives a Precommit
pure def precommit(es: DriverState, vote: Vote, vkOutput: VoteKeeperOutput): (DriverState, ConsensusOutput) =
    match vkOutput {
        | PrecommitValueVKOutput(round_value) =>
            val round = round_value._1
            val value = round_value._2
            if (es.existsProposal(round, value))
                callConsensus(es, es.bk, ProposalAndCommitAndValidConInput(vote.valueId))
            else if (round == es.cs.round)
                callConsensus(es, es.bk, PrecommitAnyConInput)
            else if (round > es.cs.round)
                // if it is for a future round I can trigger skipround
                // TODO: should we really do this. It is dead code as the f+1 event already happened
                callConsensus(es, es.bk, RoundSkipConInput(vote.round))
            // ignore messages from past rounds
            else (es, NoConsensusOutput)
        | PrecommitAnyVKOutput(round) =>
            if (round == es.cs.round)
                callConsensus(es, es.bk, PrecommitAnyConInput)
            else (es, NoConsensusOutput)
        | SkipVKOutput(round) => 
            if (round > es.cs.round)
                callConsensus(es, es.bk, RoundSkipConInput(vote.round)) 
            else (es, NoConsensusOutput)
        // none of the supported Precommit events. Do nothing
        | _ => (es, NoConsensusOutput)
    }

// We do this if the driver receives a Prevote
pure def prevote(es: DriverState, vkOutput: VoteKeeperOutput): (DriverState, ConsensusOutput) =
    // TODO: events do not have heights now.
    // TODO: Polka implications missing.
    match vkOutput {
        | PolkaValueVKOutput(round_value) =>
            val round = round_value._1
            val value = round_value._2
            if (round < es.cs.round and es.existsProposalValid(es.cs.round, value, round))
                // TODO: the value should come from the proposal   
                callConsensus(es, es.bk, ProposalAndPolkaPreviousAndValidConInput((value, round)))
            else if (round == es.cs.round)
                if (es.existsProposal(es.cs.round, value))
                    val pending = (ProposalAndPolkaAndValidConInput(value), es.cs.height, es.cs.round)
                    val newES = { ...es,  pendingInputs: es.pendingInputs.union(Set(pending)) }
                    callConsensus(newES, es.bk, PolkaAnyConInput)   
                else
                    // there is no matching proposal
                    callConsensus(es, es.bk, PolkaAnyConInput)

            // It is for a future round
            // TODO: we might check whether it is for a future round and jump
            else (es, NoConsensusOutput)

        | PolkaAnyVKOutput(round) =>
            if (round == es.cs.round)
                // call consensus and remember that we did it
                callConsensus(es, es.bk, PolkaAnyConInput)  
            // TODO: we might check whether it is for a future round and jump
            else (es, NoConsensusOutput)
                
        | PolkaNilVKOutput(round) => 
            if (round == es.cs.round)
                callConsensus(es, es.bk, PolkaNilConInput)
            else (es, NoConsensusOutput)

        | SkipVKOutput(round) => 
            if (round > es.cs.round)
                callConsensus(es, es.bk, RoundSkipConInput(round))
            else (es, NoConsensusOutput)
        
        | _ => (es, NoConsensusOutput)
    }

// We do this if a timeout expires at the driver
pure def handleTimeout(es: DriverState, t: Timeout): (DriverState, ConsensusOutput) =
    // TODO: We assume that the timeout event is always for the current round. If this is not
    // the case, we need to encode it in the input to which round the timeout belongs
    match t {
        | ProposeTimeout => callConsensus(es, es.bk, TimeoutProposeConInput((es.cs.height, es.cs.round)))
        | PrevoteTimeout => callConsensus(es, es.bk, TimeoutPrevoteConInput((es.cs.height, es.cs.round)))
        | PrecommitTimeout => callConsensus(es, es.bk, TimeoutPrecommitConInput((es.cs.height, es.cs.round)))
    }

// We do this if the driver receives a proposal
pure def handleProposal(es: DriverState, proposal: Proposal) : (DriverState, ConsensusOutput) =
    val newES = { ...es, proposals: es.proposals.union(Set(proposal))}
    if (proposal.src != Proposer(es.valset, proposal.height, proposal.round))
        // proposer does not match the height/round of the proposal
        // keep ES (don't use newES here), that is, drop proposal
        (es, NoConsensusOutput)
    else if (isValid(proposal.proposal))
        val receivedCommit = checkThreshold(    newES.bk, 
                                                proposal.round, 
                                                Precommit, 
                                                ValueThreshold(proposal.proposal))
        if (receivedCommit)
            // we have a commit that matches the proposal. We don't need to compare against
            // es.cs.round
            // TODO: check heights in bookkeeper
            // callConsensus(newES, newES.bk, ProposalAndCommitAndValidConInput(Val(id(proposal.proposal))))
            callConsensus(newES, newES.bk, ProposalAndCommitAndValidConInput(id(Val(proposal.proposal))))
        else if (proposal.round != es.cs.round or proposal.height != es.cs.height) 
            // the proposal is from the right proposer and valid, but not for this round
            // keep the proposal, do nothing else
            (newES, NoConsensusOutput)
        else
            // for current round and q, valid, and from right proposer
            val receivedPolkaValidRoundVal = checkThreshold(newES.bk,   
                                                            proposal.validRound, 
                                                            Prevote, 
                                                            ValueThreshold(id(proposal.proposal)))
            val receivedPolkaCurrentVal = checkThreshold(   newES.bk, 
                                                            newES.cs.round, 
                                                            Prevote, 
                                                            ValueThreshold(id(proposal.proposal)))
            val receivedCommitCurrentVal = checkThreshold(  newES.bk, 
                                                            newES.cs.round, 
                                                            Precommit, 
                                                            ValueThreshold(id(proposal.proposal)))
            val prevoteAndprecommitOutput = 
                if (receivedCommitCurrentVal)
                    // here we need to call both, Commit and Polka. 
                    // We do commit and append polka to pending
                    val pending = (ProposalAndPolkaAndValidConInput(id(proposal.proposal)), newES.cs.height, newES.cs.round)
                    val newES2 = { ...newES, pendingInputs: newES.pendingInputs.union(Set(pending))}
                    callConsensus(newES2, newES.bk, ProposalAndCommitAndValidConInput(id(proposal.proposal)))
                else if (receivedPolkaCurrentVal)
                    callConsensus(newES, newES.bk, ProposalAndPolkaAndValidConInput(id(proposal.proposal)))
                else (newES, NoConsensusOutput)
            match newES.cs.step { 
                | Propose =>
                    if (proposal.validRound == -1)
                        if (receivedPolkaCurrentVal)
                            callConsensus(newES, newES.bk, ProposalAndPolkaAndValidConInput(id(proposal.proposal)))
                        else
                            callConsensus(newES, newES.bk, ProposalConInput((proposal.round, id(proposal.proposal))))
                    else if (receivedPolkaValidRoundVal)
                        callConsensus(newES, newES.bk, ProposalAndPolkaPreviousAndValidConInput((id(proposal.proposal), proposal.validRound)))
                    else  
                        (newES, NoConsensusOutput)
                | Prevote => prevoteAndprecommitOutput
                | Precommit => prevoteAndprecommitOutput
                | _ => (newES, NoConsensusOutput)
            }
    else // (not(isValid(proposal))) 
        // keep ES (don't use newES here), that is, drop proposal
        match es.cs.step {
            | ProposeStep => 
                if (es.cs.round == proposal.round and es.cs.height == proposal.height)
                    if (checkThreshold(es.bk, es.cs.round, Prevote, ValueThreshold(id(proposal.proposal))))
                        callConsensus(es, es.bk, ProposalAndPolkaAndInvalidConInput((es.cs.height, es.cs.round, id(proposal.proposal))))
                    else 
                        callConsensus(es, es.bk, ProposalInvalidConInput)
                else (es, NoConsensusOutput)
            | _ => (es, NoConsensusOutput)
        }

// We do this when we need to jump to a new round
pure def skip(es: DriverState, r: Round): (DriverState, ConsensusOutput) =
    // line 15
    val prop = if (es.cs.validValue != Nil) es.cs.validValue else es.getValue()
    if (Proposer(es.valset, es.cs.height, es.cs.round + 1) == es.cs.p)
        callConsensus(es, es.bk, NewRoundProposerConInput((r, prop.getVal())))
    else 
        callConsensus(es, es.bk, NewRoundConInput(r)) 
    // what do we do now in the new round? Shouldn't we look whether we can build an event.
    // TODO: compute pending events.


// We do this when we have decided
pure def decided(es: DriverState, res: ConsensusOutput, value: Value) : (DriverState, ConsensusOutput) =
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
    ({ ...es, chain: es.chain.append(value) } , res)

// take input out of pending inputs and then call consensus with that input
// We do this when the driver is asked to work on pending events
pure def handlePendingInput(es: DriverState): (DriverState, ConsensusOutput) =
    val input_height_round = es.pendingInputs.fold((NoConsensusInput, -1, -1), (sum, y) => y)
    val newState = { ...es, pendingInputs: es.pendingInputs.exclude(Set(input_height_round))}
    if (input_height_round._2 == es.cs.height and input_height_round._3 == es.cs.round)
        callConsensus(newState, es.bk, input_height_round._1)
    else
        (newState, NoConsensusOutput)


pure def setValue(es: DriverState, value: Value) : (DriverState, ConsensusOutput) =
    ({ ...es, nextValueToPropose: value }, NoConsensusOutput)


/* *********************************************************
 * Main entry point
 * ********************************************************/

// TODO: return ConsensusInput so that we know from outside what event was fired.
pure def driver(es: DriverState, input: DriverInput) : (DriverState, ConsensusOutput) =
    // TODO: shall we check whether the sender actually is in the validator set
    match input {
        | ProposalDInput(proposal) =>
            val res = handleProposal(es, proposal)
            match res._2 {
                | DecidedOutput(value) => decided(res._1, res._2, value)
                | SkipRoundOutput(round) => skip(res._1, round)
                | _ => res
            }
        | VoteDInput(vote) => 
            match vote.step {
                | PrecommitStep =>
                    val res = applyVote(es.bk, vote, es.valset.get(vote.srcAddress), es.cs.round)
                    val newES = { ...es, bk: res.bookkeeper, voteKeeperOutput: res.event}
                    // only a commit event can come here. 
                    val cons_res = precommit(newES, vote, res.event) 
                    match cons_res._2 {
                        | DecidedOutput(value) => decided(cons_res._1, res._2, value)
                        | SkipRoundOutput(round) => skip(cons_res._1, round)
                        | _ => res
                    }
                | PrevoteStep =>
                    val res = applyVote(es.bk, vote, es.valset.get(vote.src), es.cs.round)
                    val newES = { ...es, bk: res.bookkeeper, voteKeeperOutput: res.event}
                    // only a commit event can come here. 
                    val cons_res = prevote(newES, res.event) 
                    match cons_res._2 {
                        // TODO: dead branch. But we should put this after consensus call logic into a function
                        | DecidedOutput(value) => decided(cons_res._1, res._2, value) 
                        | SkipRoundOutput(round) => skip(cons_res._1, round)
                        | _ => res
                    }
            }
        | TimeoutDInput(timeout) =>
            val res = handleTimeout(es, timeout)
            // result should be vote or skip
            if (res._2.name == "skipRound")
                skip (res._1, res._2.skipRound)
                // skip starts a new round. This may involve getValue. If we choose to move the getValue
                // logic out of the driver, we wouldn't call skip here but add a (to be defined)
                // DriverInput
            else
                res
        | StartDInput =>
            skip ({ ...es, started: true}, 0)
        | PendingDInput(_) =>
            handlePendingInput(es) // CHECK: input not used?
        | SetNextProposedValueDInput(value) =>
            setValue(es, value)
        | _ =>
            (es, NoConsensusOutput)
    }

// This is a simple function that figures out in what external events (messages,
// timeouts, etc.) the node should act.
// currently this is linked in via the state machine. But we can move it into
// the functional layer/
pure def nextAction(state: NodeState): (NodeState, DriverInput) =
    if (not(state.es.started))
        (state, StartDInput)

    else if (state.es.pendingInputs != Set())
        val input_height_round = state.es.pendingInputs.fold((NoConsensusInput, -1, -1), (sum, y) => y)
        // this might be cheating as we look into the "es"
        (state, PendingDInput(input_height_round._1))
        // I DID IT. TODO: In the "starkBFT Spec" Google doc, it is written that pending events
        // should be executed before new messages, which would requir to push this
        // branch up.

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

    else if (state.timeout.exists(x => x._2 == state.es.cs.height and 
        x._3 == state.es.cs.round))
        // pick timeout, remove it from incoming
        val timeouts = state.timeout.filter(x => x._2 == state.es.cs.height and 
            x._3 == state.es.cs.round)
        // val timeout = timeouts.chooseSome()
        val someTimeout = timeouts.fold(("", 0, 0), (sum, y) => y)
        val newstate = { ...state, timeout: state.timeout.exclude(Set(someTimeout))}
        (newstate, TimeoutDInput(someTimeout._1))

    else
        (state, NoDInput)

// This function can be used to control test runs better.
pure def nextActionCommand (state: NodeState, command: str) : (NodeState, DriverInput) = {
    if (command == "start" and not(state.es.started))
        (state, StartDInput)

    else if (command == "pending" and state.es.pendingInputs != Set())
        val input_height_round = state.es.pendingInputs.fold((NoConsensusInput, -1, -1), (sum, y) => y)
        // this might be cheating as we look into the "es"
        (state, PendingDInput(input_height_round._1))

    else if (command == "proposal" and state.incomingProposals != Set())
        // pick proposal, remove it from incoming
        // val prop = state.incomingProposals.chooseSome()
        val prop = state.incomingProposals.fold (emptyProposal, (sum, y) => y)
        val newstate = { ...state, incomingProposals: state.incomingProposals.exclude(Set(prop))}
        (newstate, ProposalDInput(prop))

    else if (command == "vote" and state.incomingVotes != Set())
        // pick vote, remove it from incoming
        // val vote = state.incomingVotes.chooseSome()
        val vote = state.incomingVotes.fold(emptyVote, (sum, y) => y)
        val newstate = { ...state, incomingVotes: state.incomingVotes.exclude(Set(vote))}
        (newstate, VoteDInput(vote))

    else if (command == "timeout" and state.timeout.exists(x => x._2 == state.es.cs.height and 
        x._3 == state.es.cs.round))
        // pick timeout, remove it from incoming
        val timeouts = state.timeout.filter(x => x._2 == state.es.cs.height and 
            x._3 == state.es.cs.round)
        // val timeout = timeouts.chooseSome()
        val someTimeout = timeouts.fold(("", 0, 0), (sum, y) => y)
        val newstate = { ...state, timeout: state.timeout.exclude(Set(someTimeout))}
        (newstate, { ...NoDInput, name: "timeout", timeout: someTimeout._1})

    else
        (state, NoDInput)
}

}
