// -*- mode: Bluespec; -*-

module votekeeperTest {

    import types.* from "../../specs/types"
    import votekeeper.* from "../../specs/votekeeper"

    // ****************************************************************************
    // Tests
    // ****************************************************************************

    // auxiliary action for tests
    action _assert(predicate: bool): bool = 
        all { assert(predicate), allUnchanged }

    // Consensus full execution with all honest validators (including the leader) and a synchronous network:
    // all messages are received in order. We assume three validators in the validator set with 60%, 30% and 10%
    // each of the total voting power
    run synchronousConsensusTest =
        initWith(1, 100)
        .then(applyVoteAction(mkVote(Prevote, "alice", 1, 1, Val("v1")), 60, 1))
        .expect(lastEmitted == NoVKOutput)
        .then(applyVoteAction(mkVote(Prevote, "john", 1, 1, Val("v1")), 10, 1))
        .expect(lastEmitted == PolkaValueVKOutput((1, "v1")))
        .then(applyVoteAction(mkVote(Prevote, "bob", 1, 1, Val("v1")), 30, 1))
        .expect(lastEmitted == NoVKOutput)
        .then(applyVoteAction(mkVote(Precommit, "bob", 1, 1, Val("v1")), 30, 1))
        .expect(lastEmitted == NoVKOutput)
        .then(applyVoteAction(mkVote(Precommit, "john", 1, 1, Val("v1")), 10, 1))
        .expect(lastEmitted == NoVKOutput)
        .then(applyVoteAction(mkVote(Precommit, "alice", 1, 1, Val("v1")), 60, 1))
        .expect(lastEmitted == PrecommitValueVKOutput((1, "v1")))

    // Reaching PolkaAny 
    run polkaAnyTest =
        initWith(1, 100)
        .then(applyVoteAction(mkVote(Prevote, "alice", 1, 1, Val("v1")), 60, 1))
        .expect(lastEmitted == NoVKOutput)
        .then(applyVoteAction(mkVote(Prevote, "john", 1, 1, Nil), 10, 1))
        .expect(lastEmitted == PolkaAnyVKOutput(1))

    // Reaching PolkaNil
    run polkaNilTest =
        initWith(1, 100)
        .then(applyVoteAction(mkVote(Prevote, "alice", 1, 1, Nil), 60, 1))
        .expect(lastEmitted == NoVKOutput)
        .then(applyVoteAction(mkVote(Prevote, "john", 1, 1, Nil), 10, 1))
        .expect(lastEmitted == PolkaNilVKOutput(1))

    // Reaching Skip via n+1 threshold with prevotes from two validators at a future round
    run skipSmallQuorumAllPrevotesTest =
        initWith(1, 100)
        .then(applyVoteAction(mkVote(Prevote, "alice", 1, 1, Val("v1")), 60, 1))
        .expect(lastEmitted == NoVKOutput)
        .then(applyVoteAction(mkVote(Prevote, "john", 1, 2, Val("v1")), 10, 1))
        .expect(lastEmitted == NoVKOutput)
        .then(applyVoteAction(mkVote(Prevote, "bob", 1, 2, Val("v1")), 30, 1))
        .expect(lastEmitted == SkipVKOutput(2))

    // Cannot reach Skip via f+1 threshold with one prevote and one precommit from the same validator at a future round
    run noSkipSmallQuorumMixedVotesSameValTest =
        initWith(1, 90)
        .then(applyVoteAction(mkVote(Prevote, "alice", 1, 1, Val("v1")), 10, 1))
        .expect(lastEmitted == NoVKOutput)
        .then(applyVoteAction(mkVote(Prevote, "john", 1, 2, Val("v1")), 20, 1))
        .expect(lastEmitted == NoVKOutput)
        .then(applyVoteAction(mkVote(Precommit, "john", 1, 2, Val("v1")), 20, 1))
        .expect(lastEmitted != SkipVKOutput(2))

    // Reaching Skip via f+1 threshold with one prevote and one precommit from two validators at a future round
    run skipSmallQuorumMixedVotesTwoValsTest =
        initWith(1, 80)
        .then(applyVoteAction(mkVote(Prevote, "alice", 1, 1, Val("v1")), 50, 1))
        .expect(lastEmitted == NoVKOutput)
        .then(applyVoteAction(mkVote(Prevote, "john", 1, 2, Val("v1")), 10, 1))
        .expect(lastEmitted == NoVKOutput)
        .then(applyVoteAction(mkVote(Prevote, "bob", 1, 2, Val("v1")), 20, 1))
        .expect(lastEmitted == SkipVKOutput(2))
        
    // Reaching Skip via 2f+1 threshold with a single prevote from a single validator at a future round
    run skipQuorumSinglePrevoteTest =
        initWith(1, 100)
        .then(applyVoteAction(mkVote(Prevote, "alice", 1, 1, Val("v1")), 10, 1))
        .expect(lastEmitted == NoVKOutput)
        .then(applyVoteAction(mkVote(Prevote, "john", 1, 2, Val("v1")), 60, 1))
        .expect(lastEmitted == SkipVKOutput(2))

    // Reaching Skip via 2f+1 threshold with a single precommit from a single validator at a future round
    run skipQuorumSinglePrecommitTest =
        initWith(1, 100)
        .then(applyVoteAction(mkVote(Prevote, "alice", 1, 1, Val("v1")), 10, 1))
        .expect(lastEmitted == NoVKOutput)
        .then(applyVoteAction(mkVote(Precommit, "john", 1, 2, Val("v1")), 60, 1))
        .expect(lastEmitted == SkipVKOutput(2))

    // Cannot reach Skip via 2f+1 threshold with one prevote and one precommit from the same validator at a future round
    run noSkipQuorumMixedVotesSameValTest =
        initWith(1, 100)
        .then(applyVoteAction(mkVote(Prevote, "alice", 1, 1, Val("v1")), 10, 1))
        .expect(lastEmitted == NoVKOutput)
        .then(applyVoteAction(mkVote(Prevote, "john", 1, 2, Val("v1")), 30, 1))
        .expect(lastEmitted == NoVKOutput)
        .then(applyVoteAction(mkVote(Precommit, "john", 1, 2, Val("v1")), 30, 1))
        .expect(lastEmitted != SkipVKOutput(2))

    // Reaching Skip via 2f+1 threshold with one prevote and one precommit from two validators at a future round
    run skipQuorumMixedVotesTwoValsTest =
        initWith(1, 80)
        .then(applyVoteAction(mkVote(Prevote, "alice", 1, 1, Val("v1")), 20, 1))
        .expect(lastEmitted == NoVKOutput)
        .then(applyVoteAction(mkVote(Prevote, "john", 1, 2, Val("v1")), 10, 1))
        .expect(lastEmitted == NoVKOutput)
        .then(applyVoteAction(mkVote(Precommit, "bob", 1, 2, Val("v1")), 50, 1))
        .expect(lastEmitted == SkipVKOutput(2))

}
