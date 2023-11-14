// -*- mode: Bluespec; -*-

module consensusTest {

import consensus.* from "./consensus"

/* ****************************************************************************
 * Global state
 * ************************************************************************* */

var system : Address_t -> ConsensusState
var _Result : ConsensusOutput
var _Event : Event


pure def initialProcess (name: Address_t) : ConsensusState = {
    { p: name, height : 1, round: -1, step: "newRound", lockedRound: -1, lockedValue: "nil", validRound: -1, validValue: "nil"}
}

action init = all {
    system' = Map ("Josef" -> initialProcess("Josef")),
    _Result' = defaultResult,
    _Event' = { name : "Initial",
                height : 0,
                round: -1,
                value: "",
                vr: -1}
}



// just to write a test.
action FireEvent(eventName: str, proc: Address_t, h: Height_t, r: Round_t, value: Value_t, vr: Round_t) : bool = all {
    val event =  {  name : eventName,
                    height : h,
                    round: r,
                    value: value,
                    vr: vr}
    val res = consensus(system.get(proc),  event )
    all {
        system' = system.put(proc, res.cs),
        _Result' = res.out,
        _Event' = event
    }
}

action step = any {
    nondet name = oneOf(ConsensusEvents)
    nondet height = 1//oneOf(1.to(4))
    nondet round = 0//oneOf(1.to(4))
    nondet value = oneOf(Set("block 1", "block 2", "block 3"))
    nondet vr = oneOf(Set(-1, 1, 2, 3, 4))
    FireEvent(name, "Josef", height, round, value, vr)
}

action unchangedAll = all {
    system' = system,
    _Result' = _Result,
    _Event' = _Event,
}

// This test should call each event at least once
run DecideNonProposerTest = {
    init
    .then(FireEvent("NewRound", "Josef", 1, 0, "", -1))
    .then(all{
        assert(_Result.timeout == "TimeoutPropose"),
        FireEvent("Proposal", "Josef", 1, 0, "block", -1)})
    .then(all{
        assert(_Result.voteMessage.step == "Prevote" and _Result.voteMessage.id == "block"),
        FireEvent("ProposalAndPolkaAndValid", "Josef", 1, 0, "block", -1)})
    .then(all{
        assert(_Result.voteMessage.step == "Precommit" and _Result.voteMessage.id == "block"),
        FireEvent("ProposalAndCommitAndValid", "Josef", 1, 0, "block", -1)})  
    .then(all{
        assert(_Result.decided == "block"),
        FireEvent("NewHeight", "Josef", system.get("Josef").height + 1, 0, "", -1)})
    .then(all{
        assert(system.get("Josef").height == 2),
        FireEvent("NewRoundProposer", "Josef", 2, 0, "nextBlock", -1)})
    .then(all{
        assert(_Result.timeout != "TimeoutPropose" and _Result.proposal.proposal == "nextBlock"),
        FireEvent("Proposal", "Josef", 2, 0, "nextBlock", -1)}) // it is assumed that the proposer receives its own message
    .then(all{
        assert(_Result.voteMessage.step == "Prevote" and system.get("Josef").step == "Prevote"),
        FireEvent("PolkaAny", "Josef", 2, 0, "nextBlock", -1)})
    .then(all{
        assert(_Result.timeout == "TimeoutPrevote"),
        FireEvent("TimeoutPrevote", "Josef", 2, 0, "nextBlock", -1)})
    .then(all{
        assert(_Result.voteMessage.step == "Precommit" and _Result.voteMessage.id == "nil" and
               system.get("Josef").step == "Precommit"),
        FireEvent("PrecommitAny", "Josef", 2, 0, "nextBlock", -1)})
    .then(all{
        assert(_Result.timeout == "TimeoutPrecommit"),
        FireEvent("TimeoutPrecommit", "Josef", 2, 0, "nextBlock", -1)})
    .then(all{
        assert(_Result.skipRound == 1),
        FireEvent("NewRound", "Josef", 2, 1, "", -1)})
    .then(all{
        assert(_Result.timeout == "TimeoutPropose"),
        FireEvent("TimeoutPropose", "Josef", 2, 1, "nextBlock", -1)})
    .then(all{
        assert(_Result.voteMessage.step == "Prevote" and _Result.voteMessage.id == "nil" and
               system.get("Josef").step == "Prevote"),
        FireEvent("PolkaNil", "Josef", 2, 1, "nextBlock", -1)})
    .then(all{
        assert(_Result.voteMessage.step == "Precommit" and _Result.voteMessage.id == "nil" and
               system.get("Josef").step == "Precommit"),
        FireEvent("PrecommitAny", "Josef", 2, 1, "nextBlock", -1)})
    .then(all{
        assert(_Result.timeout == "TimeoutPrecommit"),
        FireEvent("TimeoutPrecommit", "Josef", 2, 1, "nextBlock", -1)})
    .then(all{
        assert(_Result.skipRound == 2),
        FireEvent("NewRound", "Josef", 2, 2, "", -1)})
    .then(all{
        assert(_Result.timeout == "TimeoutPropose"),
        FireEvent("ProposalInvalid", "Josef", 2, 2, "", -1)})
    .then(all{
        assert(_Result.voteMessage.step == "Prevote" and _Result.voteMessage.id == "nil" and
               system.get("Josef").step == "Prevote"),
        unchangedAll
    })
}


}

