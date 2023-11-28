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
    const INITIAL_TOTAL_WEIGHT: Weight
    const ADDRESS_WEIGHTS: Address -> Weight // an address has a constant weight during a height
    const ROUNDS: Set[Round]
    const VALUES: Set[Value]

    // ************************************************************************
    // State machine
    // ************************************************************************

    action init = 
        initWith(INITIAL_HEIGHT, INITIAL_TOTAL_WEIGHT)
    
    action step = 
        nondet voteType = oneOf(VoteTypes)
        nondet round = oneOf(ROUNDS)
        nondet value = oneOf(VALUES.union(Set(Nil)))
        nondet address = oneOf(ADDRESS_WEIGHTS.keys())
        val height = INITIAL_HEIGHT
        val vote: Vote = { typ: voteType, height: height, round: round, value: value, address: address }
        applyVoteAction(vote, ADDRESS_WEIGHTS.get(address), 1)

}
