// ****************************************************************************
// Vote Bookkeeper State machine
// **************************************************************************** 

module voteBookkeeperSM  {

    import voteBookkeeper.* from "./voteBookkeeper"
    export voteBookkeeper.*

    // ************************************************************************
    // Model parameters 
    // ************************************************************************

    const Addresses: Set[Address]
    const Rounds: Set[Round]
    const Values: Set[Value]
    const Weights: Set[Weight]

    // ************************************************************************
    // State machine
    // ************************************************************************

    action init = 
        initWith(10, 100)
    
    action step = 
        nondet voteType = oneOf(VoteTypes)
        nondet round = oneOf(Rounds)
        nondet value = oneOf(Values)
        nondet address = oneOf(Addresses)
        nondet weight = oneOf(Weights)
        val vote: Vote = { typ: voteType, round: round, value: value, address: address }
        applyVoteAction(vote, weight)

}
