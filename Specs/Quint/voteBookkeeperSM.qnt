// ****************************************************************************
// Vote Bookkeeper State machine
// **************************************************************************** 

module voteBookkeeperSM  {

    import voteBookkeeper.* from "./voteBookkeeper"
    export voteBookkeeper.*

    // ************************************************************************
    // Model parameters 
    // ************************************************************************

    const INITIAL_HEIGHT: Height
    const INITIAL_ROUND: Round
    const INITIAL_TOTAL_WEIGHT: Weight
    const ADDRESS_WEIGHTS: Address -> Weight // an address has a constant weight during a height
    const ROUNDS: Set[Round]
    const VALUES: Set[NonNilValue]

    // ************************************************************************
    // State machine
    // ************************************************************************

    action init = 
        initWith(INITIAL_HEIGHT, INITIAL_ROUND, INITIAL_TOTAL_WEIGHT)
    
    action step = 
        nondet voteType = oneOf(Set(Prevote, Precommit))
        nondet round = oneOf(ROUNDS)
        nondet value = oneOf(VALUES.map(v => Val(v)).union(Set(Nil)))
        nondet address = oneOf(ADDRESS_WEIGHTS.keys())
        val height = INITIAL_HEIGHT
        val vote: Vote = { typ: voteType, height: height, round: round, value: value, address: address }
        applyVoteAction(vote, ADDRESS_WEIGHTS.get(address), round)

    // ****************************************************************************
    // Properties that define an expected final state (for generating traces)
    // ****************************************************************************

    val emitPrecommitValueState = 
        nondet round = oneOf(ROUNDS)
        nondet value = oneOf(VALUES)
        lastEmitted == PrecommitValueEvent((round, value))
    val emitPrecommitValue = not(emitPrecommitValueState)

    val emitPolkaAnyState = 
        nondet round = oneOf(ROUNDS)
        lastEmitted == PolkaAnyEvent(round)
    val emitPolkaAny = not(emitPolkaAnyState)

    val emitPolkaNilState = 
        nondet round = oneOf(ROUNDS)
        lastEmitted == PolkaNilEvent(round)
    val emitPolkaNil = not(emitPolkaNilState)

    val emitSkipState = 
        nondet round = oneOf(ROUNDS)
        lastEmitted == SkipEvent(round)
    val emitSkip = not(emitSkipState)

}

module voteBookkeeperModels {
    
    import voteBookkeeperSM(
        INITIAL_HEIGHT = 1,
        INITIAL_ROUND = 0,
        INITIAL_TOTAL_WEIGHT = 100,
        ADDRESS_WEIGHTS = Map("alice" -> 10, "bob" -> 30, "john" -> 60),
        ROUNDS = 0.to(2),
        VALUES = Set("v1", "v2")
    ) as VK1

}
