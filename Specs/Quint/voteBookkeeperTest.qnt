module voteBookkeeperTest {

    import voteBookkeeper.* from "./voteBookkeeper"

    // ****************************************************************************
    // State machine state
    // **************************************************************************** 

    // Bookkeeper state
    var bookkeeper: Bookkeeper
    // Last emitted event
    var lastEmitted: ExecutorEvent

    // ****************************************************************************
    // Execution
    // ****************************************************************************

    action allUnchanged: bool = all {
        bookkeeper' = bookkeeper,
        lastEmitted' = lastEmitted,
    }

    action initWith(totalWeight: Weight): bool = all {
        bookkeeper' = { height: 10, totalWeight: totalWeight, rounds: Map() },
        lastEmitted' = { round: -1, name: "", value: "null" },
    }

    action applyVoteAction(vote: Vote, weight: Weight, currentRound: Round): bool =
        val result = applyVote(bookkeeper, vote, weight, currentRound)
        all {
            bookkeeper' = result.bookkeeper,
            lastEmitted' = result.event,
        }

    // ****************************************************************************
    // Test traces
    // ****************************************************************************

    // auxiliary action for tests
    action _assert(predicate: bool): bool = 
        all { predicate, allUnchanged }

    // Consensus full execution with all honest validators (including the leader) and a synchronous network:
    // all messages are received in order. We assume three validators in the validator set wtih 60%, 30% and 10%
    // each of the total voting power
    run synchronousConsensusTest =
        initWith(100)
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "proposal", address: "alice"}, 60, 1))
        .then(_assert(lastEmitted == {round: 1, name: "None", value: "null"}))
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "proposal", address: "john"}, 10, 1))
        .then(_assert(lastEmitted == {round: 1, name: "PolkaValue", value: "proposal"}))
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "proposal", address: "bob"}, 30, 1))
        .then(_assert(lastEmitted == {round: 1, name: "None", value: "null"}))
        .then(applyVoteAction({typ: "Precommit", round: 1, value: "proposal", address: "bob"}, 30, 1))
        .then(_assert(lastEmitted == {round: 1, name: "None", value: "null"}))
        .then(applyVoteAction({typ: "Precommit", round: 1, value: "proposal", address: "john"}, 10, 1))
        .then(_assert(lastEmitted == {round: 1, name: "None", value: "null"}))
        .then(applyVoteAction({typ: "Precommit", round: 1, value: "proposal", address: "alice"}, 60, 1))
        .then(_assert(lastEmitted == {round: 1, name: "PrecommitValue", value: "proposal"}))

    // Reaching PolkaAny 
    run polkaAnyTest =
        initWith(100)
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "val1", address: "alice"}, 60, 1))
        .then(_assert(lastEmitted == {round: 1, name: "None", value: "null"}))
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "nil", address: "john"}, 10, 1))
        .then(_assert(lastEmitted == {round: 1, name: "PolkaAny", value: "null"}))

    // Reaching PolkaNil
    run polkaNilTest =
        initWith(100)
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "nil", address: "alice"}, 60, 1))
        .then(_assert(lastEmitted == {round: 1, name: "None", value: "null"}))
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "nil", address: "john"}, 10, 1))
        .then(_assert(lastEmitted == {round: 1, name: "PolkaNil", value: "null"}))

    // Reaching Skip via n+1 threshold with prevotes from two validators at a future round
    run skipSmallQuorumAllPrevotesTest =
        initWith(100)
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "proposal", address: "alice"}, 60, 1))
        .then(_assert(lastEmitted == {round: 1, name: "None", value: "null"}))
        .then(applyVoteAction({typ: "Prevote", round: 2, value: "proposal", address: "john"}, 10, 1))
        .then(_assert(lastEmitted == {round: 2, name: "None", value: "null"}))
        .then(applyVoteAction({typ: "Prevote", round: 2, value: "proposal", address: "bob"}, 30, 1))
        .then(_assert(lastEmitted == {round: 2, name: "Skip", value: "null"}))

    // Cannot reach Skip via f+1 threshold with one prevote and one precommit from the same validator at a future round
    run noSkipSmallQuorumMixedVotesSameValTest =
        initWith(90)
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "proposal", address: "alice"}, 10, 1))
        .then(_assert(lastEmitted == {round: 1, name: "None", value: "null"}))
        .then(applyVoteAction({typ: "Prevote", round: 2, value: "proposal", address: "john"}, 20, 1))
        .then(_assert(lastEmitted == {round: 2, name: "None", value: "null"}))
        .then(applyVoteAction({typ: "Precommit", round: 2, value: "proposal", address: "john"}, 20, 1))
        .then(_assert(lastEmitted != {round: 2, name: "Skip", value: "null"}))

    // Reaching Skip via f+1 threshold with one prevote and one precommit from two validators at a future round
    run skipSmallQuorumMixedVotesTwoValsTest =
        initWith(80)
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "proposal", address: "alice"}, 50, 1))
        .then(_assert(lastEmitted == {round: 1, name: "None", value: "null"}))
        .then(applyVoteAction({typ: "Prevote", round: 2, value: "proposal", address: "john"}, 10, 1))
        .then(_assert(lastEmitted == {round: 2, name: "None", value: "null"}))
        .then(applyVoteAction({typ: "Precommit", round: 2, value: "proposal", address: "bob"}, 20, 1))
        .then(_assert(lastEmitted == {round: 2, name: "Skip", value: "null"}))
        
    // Reaching Skip via 2f+1 threshold with a single prevote from a single validator at a future round
    run skipQuorumSinglePrevoteTest =
        initWith(100)
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "proposal", address: "alice"}, 10, 1))
        .then(_assert(lastEmitted == {round: 1, name: "None", value: "null"}))
        .then(applyVoteAction({typ: "Prevote", round: 2, value: "proposal", address: "john"}, 60, 1))
        .then(_assert(lastEmitted == {round: 2, name: "Skip", value: "null"}))

    // Reaching Skip via 2f+1 threshold with a single precommit from a single validator at a future round
    run skipQuorumSinglePrecommitTest =
        initWith(100)
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "proposal", address: "alice"}, 10, 1))
        .then(_assert(lastEmitted == {round: 1, name: "None", value: "null"}))
        .then(applyVoteAction({typ: "Precommit", round: 2, value: "proposal", address: "john"}, 60, 1))
        .then(_assert(lastEmitted == {round: 2, name: "Skip", value: "null"}))

    // Cannot reach Skip via 2f+1 threshold with one prevote and one precommit from the same validator at a future round
    run noSkipQuorumMixedVotesSameValTest =
        initWith(100)
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "proposal", address: "alice"}, 10, 1))
        .then(_assert(lastEmitted == {round: 1, name: "None", value: "null"}))
        .then(applyVoteAction({typ: "Prevote", round: 2, value: "proposal", address: "john"}, 30, 1))
        .then(_assert(lastEmitted == {round: 2, name: "None", value: "null"}))
        .then(applyVoteAction({typ: "Precommit", round: 2, value: "proposal", address: "john"}, 30, 1))
        .then(_assert(lastEmitted != {round: 2, name: "Skip", value: "null"}))

    // Reaching Skip via 2f+1 threshold with one prevote and one precommit from two validators at a future round
    run skipQuorumMixedVotesTwoValsTest =
        initWith(80)
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "proposal", address: "alice"}, 20, 1))
        .then(_assert(lastEmitted == {round: 1, name: "None", value: "null"}))
        .then(applyVoteAction({typ: "Prevote", round: 2, value: "proposal", address: "john"}, 10, 1))
        .then(_assert(lastEmitted == {round: 2, name: "None", value: "null"}))
        .then(applyVoteAction({typ: "Precommit", round: 2, value: "proposal", address: "bob"}, 50, 1))
        .then(_assert(lastEmitted == {round: 2, name: "Skip", value: "null"}))

}
