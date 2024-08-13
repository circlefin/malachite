// -*- mode: Bluespec; -*-

// ****************************************************************************
// Vote Bookkeeper State Machine
// **************************************************************************** 

module votekeeperSM  {

    import types.* from "../../specs/types"
    import votekeeper.* from "../../specs/votekeeper"
    export votekeeper.*

    // ************************************************************************
    // Model parameters 
    // ************************************************************************

    const INITIAL_HEIGHT: Height
    const INITIAL_TOTAL_WEIGHT: Weight
    const ADDRESS_WEIGHTS: Address -> Weight // an address has a constant weight during a height
    const ROUNDS: Set[Round]
    const VALUES: Set[NonNilValue]

    // ************************************************************************
    // State machine
    // ************************************************************************

    action init = 
        initWith(INITIAL_HEIGHT, ADDRESS_WEIGHTS)
    
    action step = 
        nondet voteType = oneOf(Set(Prevote, Precommit))
        nondet round = oneOf(ROUNDS)
        nondet value = oneOf(VALUES.map(v => Val(v)).union(Set(Nil)))
        nondet address = oneOf(ADDRESS_WEIGHTS.keys())
        val height = INITIAL_HEIGHT
        val vote: Vote = mkVote(voteType, address, height, round, id(value))
        applyVoteAction(vote, round)

    // ****************************************************************************
    // Properties that define an expected final state (for generating traces)
    // ****************************************************************************

    val emitPrecommitValueState = 
        nondet round = oneOf(ROUNDS)
        nondet value = oneOf(VALUES)
        lastEmitted == PrecommitValueVKOutput((round, value))
    val emitPrecommitValue = not(emitPrecommitValueState)

    val emitPolkaAnyState = 
        nondet round = oneOf(ROUNDS)
        lastEmitted == PolkaAnyVKOutput(round)
    val emitPolkaAny = not(emitPolkaAnyState)

    val emitPolkaNilState = 
        nondet round = oneOf(ROUNDS)
        lastEmitted == PolkaNilVKOutput(round)
    val emitPolkaNil = not(emitPolkaNilState)

    val emitSkipState = 
        nondet round = oneOf(ROUNDS)
        lastEmitted == SkipVKOutput(round)
    val emitSkip = not(emitSkipState)

}

// ****************************************************************************
// Vote Bookkeeper Models
// **************************************************************************** 

module votekeeperModels {
    
    import votekeeperSM(
        INITIAL_HEIGHT = 1,
        INITIAL_TOTAL_WEIGHT = 100,
        ADDRESS_WEIGHTS = Map("alice" -> 10, "bob" -> 30, "john" -> 60),
        ROUNDS = 0.to(2),
        VALUES = Set("v1", "v2")
    ) as VK1

}
