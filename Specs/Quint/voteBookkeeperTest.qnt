module voteBookkeeperTest {

    import voteBookkeeperSM(
        Addresses = Set("alice", "bob", "john"),
        Rounds = 1.to(1),
        Values = Set("nil", "proposal"),
        Weights = Set(10, 30, 60, 100)
    ).* from "./voteBookkeeperSM"

    // ****************************************************************************
    // Tests
    // ****************************************************************************

    // auxiliary action for tests
    action _assert(predicate: bool): bool = 
        all { predicate, allUnchanged }

    // Consensus full execution with all honest validators (including the leader) and a synchronous network:
    // all messages are received in order. We assume three validators in the validator set wtih 60%, 30% and 10%
    // each of the total voting power
    run synchronousConsensusTest =
        init
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "proposal", address: "alice"}, 60))
        .then(_assert(lastEmitted == noEvent(1)))
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "proposal", address: "john"}, 10))
        .then(_assert(lastEmitted == polkaValueEvent(1, "proposal")))
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "proposal", address: "bob"}, 30))
        .then(_assert(lastEmitted == noEvent(1)))
        .then(applyVoteAction({typ: "Precommit", round: 1, value: "proposal", address: "bob"}, 30))
        .then(_assert(lastEmitted == noEvent(1)))
        .then(applyVoteAction({typ: "Precommit", round: 1, value: "proposal", address: "john"}, 10))
        .then(_assert(lastEmitted == noEvent(1)))
        .then(applyVoteAction({typ: "Precommit", round: 1, value: "proposal", address: "alice"}, 60))
        .then(_assert(lastEmitted == precommitValueEvent(1, "proposal")))

    // Reaching PolkaAny 
    run polkaAnyTest =
        init
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "val1", address: "alice"}, 60))
        .then(_assert(lastEmitted == noEvent(1)))
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "nil", address: "john"}, 10))
        .then(_assert(lastEmitted == polkaAnyEvent(1)))

    // Reaching PolkaNil
    run polkaNilTest =
        init
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "nil", address: "alice"}, 60))
        .then(_assert(lastEmitted == noEvent(1)))
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "nil", address: "john"}, 10))
        .then(_assert(lastEmitted == polkaNilEvent(1)))

    // ****************************************************************************
    // Properties that define an expected final state (for generating traces)
    // ****************************************************************************

    val emitPrecommitValueState = lastEmitted.name == "PrecommitValue"
    val emitPrecommitValue = not(emitPrecommitValueState)

    val emitPolkaAnyState = lastEmitted.name == "PolkaAny"
    val emitPolkaAny = not(emitPolkaAnyState)

    val emitPolkaNilState = lastEmitted.name == "PolkaNil"
    val emitPolkaNil = not(emitPolkaNilState)

}
