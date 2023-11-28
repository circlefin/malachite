module voteBookkeeperTest {

    import voteBookkeeperSM(
        INITIAL_HEIGHT = 1,
        INITIAL_TOTAL_WEIGHT = 100,
        ADDRESS_WEIGHTS = Map("alice" -> 10, "bob" -> 30, "john" -> 60),
        ROUNDS = 1.to(2),
        VALUES = Set("val1", "val2")
    ).* from "./voteBookkeeperSM"

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
        .then(applyVoteAction({typ: "Prevote", height: 1, round: 1, value: "val1", address: "alice"}, 60, 1))
        .then(_assert(lastEmitted == noEvent(1)))
        .then(applyVoteAction({typ: "Prevote", height: 1, round: 1, value: "val1", address: "john"}, 10, 1))
        .then(_assert(lastEmitted == polkaValueEvent(1, "val1")))
        .then(applyVoteAction({typ: "Prevote", height: 1, round: 1, value: "val1", address: "bob"}, 30, 1))
        .then(_assert(lastEmitted == noEvent(1)))
        .then(applyVoteAction({typ: "Precommit", height: 1, round: 1, value: "val1", address: "bob"}, 30, 1))
        .then(_assert(lastEmitted == noEvent(1)))
        .then(applyVoteAction({typ: "Precommit", height: 1, round: 1, value: "val1", address: "john"}, 10, 1))
        .then(_assert(lastEmitted == noEvent(1)))
        .then(applyVoteAction({typ: "Precommit", height: 1, round: 1, value: "val1", address: "alice"}, 60, 1))
        .then(_assert(lastEmitted == precommitValueEvent(1, "val1")))

    // Reaching PolkaAny 
    run polkaAnyTest =
        initWith(1, 100)
        .then(applyVoteAction({typ: "Prevote", height: 1, round: 1, value: "val1", address: "alice"}, 60, 1))
        .then(_assert(lastEmitted == noEvent(1)))
        .then(applyVoteAction({typ: "Prevote", height: 1, round: 1, value: "nil", address: "john"}, 10, 1))
        .then(_assert(lastEmitted == polkaAnyEvent(1)))

    // Reaching PolkaNil
    run polkaNilTest =
        initWith(1, 100)
        .then(applyVoteAction({typ: "Prevote", height: 1, round: 1, value: "nil", address: "alice"}, 60, 1))
        .then(_assert(lastEmitted == noEvent(1)))
        .then(applyVoteAction({typ: "Prevote", height: 1, round: 1, value: "nil", address: "john"}, 10, 1))
        .then(_assert(lastEmitted == polkaNilEvent(1)))

    // Reaching Skip via n+1 threshold with prevotes from two validators at a future round
    run skipSmallQuorumAllPrevotesTest =
        initWith(1, 100)
        .then(applyVoteAction({typ: "Prevote", height: 1, round: 1, value: "val1", address: "alice"}, 60, 1))
        .then(_assert(lastEmitted == noEvent(1)))
        .then(applyVoteAction({typ: "Prevote", height: 1, round: 2, value: "val1", address: "john"}, 10, 1))
        .then(_assert(lastEmitted == noEvent(2)))
        .then(applyVoteAction({typ: "Prevote", height: 1, round: 2, value: "val1", address: "bob"}, 30, 1))
        .then(_assert(lastEmitted == skipEvent(2)))

    // Cannot reach Skip via f+1 threshold with one prevote and one precommit from the same validator at a future round
    run noSkipSmallQuorumMixedVotesSameValTest =
        initWith(1, 90)
        .then(applyVoteAction({typ: "Prevote", height: 1, round: 1, value: "val1", address: "alice"}, 10, 1))
        .then(_assert(lastEmitted == noEvent(1)))
        .then(applyVoteAction({typ: "Prevote", height: 1, round: 2, value: "val1", address: "john"}, 20, 1))
        .then(_assert(lastEmitted == noEvent(2)))
        .then(applyVoteAction({typ: "Precommit", height: 1, round: 2, value: "val1", address: "john"}, 20, 1))
        .then(_assert(lastEmitted != skipEvent(2)))

    // Reaching Skip via f+1 threshold with one prevote and one precommit from two validators at a future round
    run skipSmallQuorumMixedVotesTwoValsTest =
        initWith(1, 80)
        .then(applyVoteAction({typ: "Prevote", height: 1, round: 1, value: "val1", address: "alice"}, 50, 1))
        .then(_assert(lastEmitted == noEvent(1)))
        .then(applyVoteAction({typ: "Prevote", height: 1, round: 2, value: "val1", address: "john"}, 10, 1))
        .then(_assert(lastEmitted == noEvent(2)))
        .then(applyVoteAction({typ: "Precommit", height: 1, round: 2, value: "val1", address: "bob"}, 20, 1))
        .then(_assert(lastEmitted == skipEvent(2)))

    // Reaching Skip via 2f+1 threshold with a single prevote from a single validator at a future round
    run skipQuorumSinglePrevoteTest =
        initWith(1, 100)
        .then(applyVoteAction({typ: "Prevote", height: 1, round: 1, value: "val1", address: "alice"}, 10, 1))
        .then(all { assert(lastEmitted == noEvent(1)), allUnchanged })
        .then(applyVoteAction({typ: "Prevote", height: 1, round: 2, value: "val1", address: "john"}, 60, 1))
        .then(all { assert(lastEmitted == skipEvent(2)), allUnchanged })

    // Reaching Skip via 2f+1 threshold with a single precommit from a single validator at a future round
    run skipQuorumSinglePrecommitTest =
        initWith(1, 100)
        .then(applyVoteAction({typ: "Prevote", height: 1, round: 1, value: "val1", address: "alice"}, 10, 1))
        .then(_assert(lastEmitted == {round: 1, name: "None", value: "null"}))
        .then(applyVoteAction({typ: "Precommit", height: 1, round: 2, value: "val1", address: "john"}, 60, 1))
        .then(_assert(lastEmitted == skipEvent(2)))

    // Cannot reach Skip via 2f+1 threshold with one prevote and one precommit from the same validator at a future round
    run noSkipQuorumMixedVotesSameValTest =
        initWith(1, 100)
        .then(applyVoteAction({typ: "Prevote", height: 1, round: 1, value: "val1", address: "alice"}, 10, 1))
        .then(_assert(lastEmitted == noEvent(1)))
        .then(applyVoteAction({typ: "Prevote", height: 1, round: 2, value: "val1", address: "john"}, 30, 1))
        .then(_assert(lastEmitted == noEvent(2)))
        .then(applyVoteAction({typ: "Precommit", height: 1, round: 2, value: "val1", address: "john"}, 30, 1))
        .then(_assert(lastEmitted != skipEvent(2)))

    // Reaching Skip via 2f+1 threshold with one prevote and one precommit from two validators at a future round
    run skipQuorumMixedVotesTwoValsTest =
        initWith(1, 80)
        .then(applyVoteAction({typ: "Prevote", height: 1, round: 1, value: "val1", address: "alice"}, 20, 1))
        .then(_assert(lastEmitted == noEvent(1)))
        .then(applyVoteAction({typ: "Prevote", height: 1, round: 2, value: "val1", address: "john"}, 10, 1))
        .then(_assert(lastEmitted == noEvent(2)))
        .then(applyVoteAction({typ: "Precommit", height: 1, round: 2, value: "val1", address: "bob"}, 50, 1))
        .then(_assert(lastEmitted == skipEvent(2)))

    // ****************************************************************************
    // Properties that define an expected final state (for generating traces)
    // ****************************************************************************

    val emitPrecommitValueState = lastEmitted.name == "PrecommitValue"
    val emitPrecommitValue = not(emitPrecommitValueState)

    val emitPolkaAnyState = lastEmitted.name == "PolkaAny"
    val emitPolkaAny = not(emitPolkaAnyState)

    val emitPolkaNilState = lastEmitted.name == "PolkaNil"
    val emitPolkaNil = not(emitPolkaNilState)

    val emitSkipState = lastEmitted.name == "Skip"
    val emitSkip = not(emitSkipState)
}
