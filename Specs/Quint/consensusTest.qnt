// -*- mode: Bluespec; -*-

module consensusTest {

import consensus.* from "./consensus"

/* ****************************************************************************
 * Global state
 * ************************************************************************* */

var system: Address_t -> ConsensusState
var _output: ConsensusOutput
var _input: ConsensusInput


pure def initialProcess (name: Address_t) : ConsensusState = {
    { p: name, height : 1, round: -1, step: "newRound", lockedRound: -1, lockedValue: "nil", validRound: -1, validValue: "nil"}
}

action init = all {
    system' = Map ("Josef" -> initialProcess("Josef")),
    _output' = defaultOutput,
    _input' = { name : "Initial",
                height : 0,
                round: -1,
                value: "",
                vr: -1}
}



// just to write a test.
action FireInput(eventName: str, proc: Address_t, h: Height_t, r: Round_t, value: Value_t, vr: Round_t) : bool = all {
    val event =  {  name : eventName,
                    height : h,
                    round: r,
                    value: value,
                    vr: vr}
    val res = consensus(system.get(proc),  event )
    all {
        system' = system.put(proc, res.cs),
        _output' = res.out,
        _input' = event
    }
}

action step = any {
    nondet name = oneOf(ConsensusInputNames)
    nondet height = 1//oneOf(1.to(4))
    nondet round = 0//oneOf(1.to(4))
    nondet value = oneOf(Set("block 1", "block 2", "block 3"))
    nondet vr = oneOf(Set(-1, 1, 2, 3, 4))
    FireInput(name, "Josef", height, round, value, vr)
}

action unchangedAll = all {
    system' = system,
    _output' = _output,
    _input' = _input,
}

// This test should call each event at least once
run DecideNonProposerTest = {
    init
    .then(FireInput("NewRound", "Josef", 1, 0, "", -1))
    .then(all{
        assert(_output.timeout == "TimeoutPropose"),
        FireInput("Proposal", "Josef", 1, 0, "block", -1)})
    .then(all{
        assert(_output.voteMessage.step == "Prevote" and _output.voteMessage.id == "block"),
        FireInput("ProposalAndPolkaAndValid", "Josef", 1, 0, "block", -1)})
    .then(all{
        assert(_output.voteMessage.step == "Precommit" and _output.voteMessage.id == "block"),
        FireInput("ProposalAndCommitAndValid", "Josef", 1, 0, "block", -1)})  
    .then(all{
        assert(_output.decided == "block"),
        FireInput("NewHeight", "Josef", system.get("Josef").height + 1, 0, "", -1)})
    .then(all{
        assert(system.get("Josef").height == 2),
        FireInput("NewRoundProposer", "Josef", 2, 0, "nextBlock", -1)})
    .then(all{
        assert(_output.timeout != "TimeoutPropose" and _output.proposal.proposal == "nextBlock"),
        FireInput("Proposal", "Josef", 2, 0, "nextBlock", -1)}) // it is assumed that the proposer receives its own message
    .then(all{
        assert(_output.voteMessage.step == "Prevote" and system.get("Josef").step == "Prevote"),
        FireInput("PolkaAny", "Josef", 2, 0, "nextBlock", -1)})
    .then(all{
        assert(_output.timeout == "TimeoutPrevote"),
        FireInput("TimeoutPrevote", "Josef", 2, 0, "nextBlock", -1)})
    .then(all{
        assert(_output.voteMessage.step == "Precommit" and _output.voteMessage.id == "nil" and
               system.get("Josef").step == "Precommit"),
        FireInput("PrecommitAny", "Josef", 2, 0, "nextBlock", -1)})
    .then(all{
        assert(_output.timeout == "TimeoutPrecommit"),
        FireInput("TimeoutPrecommit", "Josef", 2, 0, "nextBlock", -1)})
    .then(all{
        assert(_output.skipRound == 1),
        FireInput("NewRound", "Josef", 2, 1, "", -1)})
    .then(all{
        assert(_output.timeout == "TimeoutPropose"),
        FireInput("TimeoutPropose", "Josef", 2, 1, "nextBlock", -1)})
    .then(all{
        assert(_output.voteMessage.step == "Prevote" and _output.voteMessage.id == "nil" and
               system.get("Josef").step == "Prevote"),
        FireInput("PolkaNil", "Josef", 2, 1, "nextBlock", -1)})
    .then(all{
        assert(_output.voteMessage.step == "Precommit" and _output.voteMessage.id == "nil" and
               system.get("Josef").step == "Precommit"),
        FireInput("PrecommitAny", "Josef", 2, 1, "nextBlock", -1)})
    .then(all{
        assert(_output.timeout == "TimeoutPrecommit"),
        FireInput("TimeoutPrecommit", "Josef", 2, 1, "nextBlock", -1)})
    .then(all{
        assert(_output.skipRound == 2),
        FireInput("NewRound", "Josef", 2, 2, "", -1)})
    .then(all{
        assert(_output.timeout == "TimeoutPropose"),
        FireInput("ProposalInvalid", "Josef", 2, 2, "", -1)})
    .then(all{
        assert(_output.voteMessage.step == "Prevote" and _output.voteMessage.id == "nil" and
               system.get("Josef").step == "Prevote"),
        unchangedAll
    })
}


}

