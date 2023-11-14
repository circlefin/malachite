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

    action init(totalWeight: Weight): bool = all {
        bookkeeper' = { height: 10, totalWeight: totalWeight, rounds: Map() },
        lastEmitted' = { round: -1, name: "", value: "null" },
    }

    action applyVoteAction(vote: Vote, weight: Weight): bool =
        val result = applyVote(bookkeeper, vote, weight)
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
        init(100)
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "proposal", address: "alice"}, 60))
        .then(_assert(lastEmitted == {round: 1, name: "None", value: "null"}))
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "proposal", address: "john"}, 10))
        .then(_assert(lastEmitted == {round: 1, name: "PolkaValue", value: "proposal"}))
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "proposal", address: "bob"}, 30))
        .then(_assert(lastEmitted == {round: 1, name: "None", value: "null"}))
        .then(applyVoteAction({typ: "Precommit", round: 1, value: "proposal", address: "bob"}, 30))
        .then(_assert(lastEmitted == {round: 1, name: "None", value: "null"}))
        .then(applyVoteAction({typ: "Precommit", round: 1, value: "proposal", address: "john"}, 10))
        .then(_assert(lastEmitted == {round: 1, name: "None", value: "null"}))
        .then(applyVoteAction({typ: "Precommit", round: 1, value: "proposal", address: "alice"}, 60))
        .then(_assert(lastEmitted == {round: 1, name: "PrecommitValue", value: "proposal"}))

    // Reaching PolkaAny 
    run polkaAnyTest =
        init(100)
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "val1", address: "alice"}, 60))
        .then(_assert(lastEmitted == {round: 1, name: "None", value: "null"}))
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "nil", address: "john"}, 10))
        .then(_assert(lastEmitted == {round: 1, name: "PolkaAny", value: "null"}))

    // Reaching PolkaNil
    run polkaNilTest =
        init(100)
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "nil", address: "alice"}, 60))
        .then(_assert(lastEmitted == {round: 1, name: "None", value: "null"}))
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "nil", address: "john"}, 10))
        .then(_assert(lastEmitted == {round: 1, name: "PolkaNil", value: "null"}))

}
