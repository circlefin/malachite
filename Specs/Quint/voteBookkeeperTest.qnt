module voteBookkeeperTest {

    import voteBookkeeper.* from "./voteBookkeeper"
    export voteBookkeeper.*

    // ****************************************************************************
    // Test traces
    // ****************************************************************************

    // auxiliary action for tests
    action _assert(predicate: bool): bool = 
        all { assert(predicate), allUnchanged }

    // Consensus full execution with all honest validators (including the leader) and a synchronous network:
    // all messages are received in order. We assume three validators in the validator set with 60%, 30% and 10%
    // each of the total voting power
    run synchronousConsensusTest =
        initWith(1, 1, 100)
        .then(applyVoteAction({typ: Prevote, height: 1, round: 1, value: Val("v1"), address: "alice"}, 60, 1))
        .then(_assert(lastEmitted == NoEvent(1)))
        .then(applyVoteAction({typ: Prevote, height: 1, round: 1, value: Val("v1"), address: "john"}, 10, 1))
        .then(_assert(lastEmitted == PolkaValueEvent((1, Val("v1")))))
        .then(applyVoteAction({typ: Prevote, height: 1, round: 1, value: Val("v1"), address: "bob"}, 30, 1))
        .then(_assert(lastEmitted == NoEvent(1)))
        .then(applyVoteAction({typ: Precommit, height: 1, round: 1, value: Val("v1"), address: "bob"}, 30, 1))
        .then(_assert(lastEmitted == NoEvent(1)))
        .then(applyVoteAction({typ: Precommit, height: 1, round: 1, value: Val("v1"), address: "john"}, 10, 1))
        .then(_assert(lastEmitted == NoEvent(1)))
        .then(applyVoteAction({typ: Precommit, height: 1, round: 1, value: Val("v1"), address: "alice"}, 60, 1))
        .then(_assert(lastEmitted == PrecommitValueEvent((1, Val("v1")))))

    // Reaching PolkaAny 
    run polkaAnyTest =
        initWith(1, 1, 100)
        .then(applyVoteAction({typ: Prevote, height: 1,round: 1, value: Val("v1"), address: "alice"}, 60, 1))
        .then(_assert(lastEmitted == NoEvent(1)))
        .then(applyVoteAction({typ: Prevote, height: 1, round: 1, value: Nil, address: "john"}, 10, 1))
        .then(_assert(lastEmitted == PolkaAnyEvent(1)))

    // Reaching PolkaNil
    run polkaNilTest =
        initWith(1, 1, 100)
        .then(applyVoteAction({typ: Prevote, height: 1, round: 1, value: Nil, address: "alice"}, 60, 1))
        .then(_assert(lastEmitted == NoEvent(1)))
        .then(applyVoteAction({typ: Prevote, height: 1, round: 1, value: Nil, address: "john"}, 10, 1))
        .then(_assert(lastEmitted == PolkaNilEvent(1)))

    // Reaching Skip via n+1 threshold with prevotes from two validators at a future round
    run skipSmallQuorumAllPrevotesTest =
        initWith(1, 1, 100)
        .then(applyVoteAction({typ: Prevote, height: 1, round: 1, value: Val("v1"), address: "alice"}, 60, 1))
        .then(_assert(lastEmitted == NoEvent(1)))
        .then(applyVoteAction({typ: Prevote, height: 1, round: 2, value: Val("v1"), address: "john"}, 10, 1))
        .then(_assert(lastEmitted == NoEvent(2)))
        .then(applyVoteAction({typ: Prevote, height: 1, round: 2, value: Val("v1"), address: "bob"}, 30, 1))
        .then(_assert(lastEmitted == SkipEvent(2)))

    // Cannot reach Skip via f+1 threshold with one prevote and one precommit from the same validator at a future round
    run noSkipSmallQuorumMixedVotesSameValTest =
        initWith(1, 1, 90)
        .then(applyVoteAction({typ: Prevote, height: 1, round: 1, value: Val("v1"), address: "alice"}, 10, 1))
        .then(_assert(lastEmitted == NoEvent(1)))
        .then(applyVoteAction({typ: Prevote, height: 1, round: 2, value: Val("v1"), address: "john"}, 20, 1))
        .then(_assert(lastEmitted == NoEvent(2)))
        .then(applyVoteAction({typ: Precommit, height: 1, round: 2, value: Val("v1"), address: "john"}, 20, 1))
        .then(_assert(lastEmitted != SkipEvent(2)))

    // Reaching Skip via f+1 threshold with one prevote and one precommit from two validators at a future round
    run skipSmallQuorumMixedVotesTwoValsTest =
        initWith(1, 1, 80)
        .then(applyVoteAction({typ: Prevote, height: 1, round: 1, value: Val("v1"), address: "alice"}, 50, 1))
        .then(_assert(lastEmitted == NoEvent(1)))
        .then(applyVoteAction({typ: Prevote, height: 1, round: 2, value: Val("v1"), address: "john"}, 10, 1))
        .then(_assert(lastEmitted == NoEvent(2)))
        .then(applyVoteAction({typ: Precommit, height: 1, round: 2, value: Val("v1"), address: "bob"}, 20, 1))
        .then(_assert(lastEmitted == SkipEvent(2)))
        
    // Reaching Skip via 2f+1 threshold with a single prevote from a single validator at a future round
    run skipQuorumSinglePrevoteTest =
        initWith(1, 1, 100)
        .then(applyVoteAction({typ: Prevote, height: 1, round: 1, value: Val("v1"), address: "alice"}, 10, 1))
        .then(_assert(lastEmitted == NoEvent(1)))
        .then(applyVoteAction({typ: Prevote, height: 1, round: 2, value: Val("v1"), address: "john"}, 60, 1))
        .then(_assert(lastEmitted == SkipEvent(2)))

    // Reaching Skip via 2f+1 threshold with a single precommit from a single validator at a future round
    run skipQuorumSinglePrecommitTest =
        initWith(1, 1, 100)
        .then(applyVoteAction({typ: Prevote, height: 1, round: 1, value: Val("v1"), address: "alice"}, 10, 1))
        .then(_assert(lastEmitted == NoEvent(1)))
        .then(applyVoteAction({typ: Precommit, height: 1, round: 2, value: Val("v1"), address: "john"}, 60, 1))
        .then(_assert(lastEmitted == SkipEvent(2)))

    // Cannot reach Skip via 2f+1 threshold with one prevote and one precommit from the same validator at a future round
    run noSkipQuorumMixedVotesSameValTest =
        initWith(1, 1, 100)
        .then(applyVoteAction({typ: Prevote, height: 1, round: 1, value: Val("v1"), address: "alice"}, 10, 1))
        .then(_assert(lastEmitted == NoEvent(1)))
        .then(applyVoteAction({typ: Prevote, height: 1, round: 2, value: Val("v1"), address: "john"}, 30, 1))
        .then(_assert(lastEmitted == NoEvent(2)))
        .then(applyVoteAction({typ: Precommit, height: 1, round: 2, value: Val("v1"), address: "john"}, 30, 1))
        .then(_assert(lastEmitted != SkipEvent(2)))

    // Reaching Skip via 2f+1 threshold with one prevote and one precommit from two validators at a future round
    run skipQuorumMixedVotesTwoValsTest =
        initWith(1, 1, 80)
        .then(applyVoteAction({typ: Prevote, height: 1, round: 1, value: Val("v1"), address: "alice"}, 20, 1))
        .then(_assert(lastEmitted == NoEvent(1)))
        .then(applyVoteAction({typ: Prevote, height: 1, round: 2, value: Val("v1"), address: "john"}, 10, 1))
        .then(_assert(lastEmitted == NoEvent(2)))
        .then(applyVoteAction({typ: Precommit, height: 1, round: 2, value: Val("v1"), address: "bob"}, 50, 1))
        .then(_assert(lastEmitted == SkipEvent(2)))

}
