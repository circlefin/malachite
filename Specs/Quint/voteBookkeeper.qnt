// -*- mode: Bluespec; -*-

module voteBookkeeper {

    import basicSpells.* from "./basicSpells"
    import extraSpells.* from "./extraSpells"

    // ****************************************************************************
    // Types
    // ****************************************************************************

    // An address is an string
    type Address = str

    // A round is an integer
    type Round = int

    // A height is an integer
    type Height = int

    // A value is either nil or a string
    type NonNilValue = str
    type Value = Nil | Val(NonNilValue)

    // Weight is an integer
    type Weight = int

    type VoteType = Prevote | Precommit

    type Vote = {
        typ: VoteType,
        round: Round,
        value: Value,
        address: Address
    }

    type VoteCount = {
        totalWeight: Weight,
        valuesWeights: Value -> Weight,
        votesAddresses: Set[Address]
    }

    type RoundVotes = {
        height: Height,
        round: Round,
        prevotes: VoteCount,
        precommits: VoteCount,
        emittedEvents: Set[ExecutorEvent],
        votesAddressesWeights: Address -> Weight
    }

    type Threshold =
        | UnreachedThreshold
        | NilThreshold
        | AnyThreshold
        | SkipThreshold
        | ValueThreshold(NonNilValue)

    type ExecutorEvent = 
        | NoEvent(Round)
        | PolkaAnyEvent(Round)
        | PolkaNilEvent(Round)
        | PolkaValueEvent((Round, Value))
        | PrecommitAnyEvent(Round)
        | PrecommitValueEvent((Round, Value))
        | SkipEvent(Round)

    def isNoEvent(event) = match event { 
        | NoEvent(_) => true
        | PolkaAnyEvent(_) => false
        | PolkaNilEvent(_) => false
        | PolkaValueEvent(_) => false
        | PrecommitAnyEvent(_) => false
        | PrecommitValueEvent(_) => false
        | SkipEvent(_) => false
    }

    type Bookkeeper = {
        height: Height,
        currentRound: Round,
        totalWeight: Weight,
        rounds: Round -> RoundVotes
    }

    // ****************************************************************************
    // Functional Layer
    // ****************************************************************************

    // Internal functions

    // creates a new voteCount
    pure def newVoteCount(total: Weight): VoteCount =
        { totalWeight: total, valuesWeights: Map(), votesAddresses: Set() }

    // Returns true if weight > 2/3 * total (quorum: at least f+1 correct)
    pure def isQuorum(weight: Weight, total: Weight): bool =
        3 * weight > 2 * total

    // True iff the vote count has a quorum on a specific value.
    pure def hasQuorumOnValue(voteCount: VoteCount, value: Value): bool =
        isQuorum(voteCount.valuesWeights.getOrElse(value, 0), voteCount.totalWeight)

    // True iff the vote count has a quorum on any value.
    pure def hasQuorumOnAny(voteCount: VoteCount): bool =
        isQuorum(voteCount.valuesWeights.mapSumValues(), voteCount.totalWeight)

    // Returns true if weight > 1/3 * total (small quorum: at least one correct)
    pure def isSkip(weight: Weight, total: Weight): bool =
        3 * weight > total

    // Adds a vote of weight weigth to a voteCount if there is not vote registered for the voter.
    pure def addVote(voteCount: VoteCount, vote: Vote, weight: Weight): VoteCount =
        if (vote.address.in(voteCount.votesAddresses))
            // Do not count vote if address has already voted.
            voteCount
        else 
            val newWeight = voteCount.valuesWeights.getOrElse(vote.value, 0) + weight             
            voteCount
                .with("valuesWeights", voteCount.valuesWeights.mapSafeSet(vote.value, newWeight))
                .with("votesAddresses", voteCount.votesAddresses.setAdd(vote.address))

    // Given a voteCount and a value, the function returns:
    // - A threshold Value if there is a quorum for the given value;
    // - A threshold Nil if there is a quorum for the nil and no quorum for the value;
    // - A threshold Any if there is no quorum for the value or nil and there is a quroum for any (including nil);
    // - A threshold Unreached otherwise indicating that no quorum has been yet reached.
    pure def computeThreshold(voteCount: VoteCount, value: Value): Threshold =
        if (voteCount.hasQuorumOnValue(value)) {
            match value {
                | Nil => NilThreshold
                | Val(v) => ValueThreshold(v)
            }
        } else if (voteCount.hasQuorumOnAny()) {
            AnyThreshold
        } else 
            UnreachedThreshold

    // Given a round, voteType and threshold, return the corresponding ExecutorEvent
    pure def toEvent(round: Round, voteType: VoteType, threshold: Threshold): ExecutorEvent =
        match threshold {
            | UnreachedThreshold => 
                NoEvent(round)
            | ValueThreshold(value) =>
                match voteType {
                    | Prevote => PolkaValueEvent((round, Val(value)))
                    | Precommit => PrecommitValueEvent((round, Val(value)))
                }
            | NilThreshold =>
                match voteType {
                    | Prevote => PolkaNilEvent(round)
                    | Precommit => PrecommitAnyEvent(round)
                }
            | AnyThreshold =>
                match voteType {
                    | Prevote => PolkaAnyEvent(round)
                    | Precommit => PrecommitAnyEvent(round)
                }
            | SkipThreshold => 
                SkipEvent(round)
        }

    // Executor interface
    type BKResult = { bookkeeper: Bookkeeper, event: ExecutorEvent }

    // Called by the executor when it receives a vote. The function takes the following steps:
    // - It first adds the vote and then computes a threshold.
    // - If there exist a threshold and has not emitted before, the function returns the corresponsing ExecutorEvent.
    // - Othewise, the function returns a no-threshold event.
    // - Note that if there is no threshold after adding the vote, the function checks if there is a skip threshold.
    // TO DISCUSS:
    // - There might be a problem if we generalize from single-shot to multi-shot: the keeper only keeps the totalWeight
    //   of the current height; I wonder if we need to keep the totalWeight for every Height that we may receive a vote for.
    pure def applyVote(keeper: Bookkeeper, vote: Vote, weight: Weight): { bookkeeper: Bookkeeper, event: ExecutorEvent } =
        val height = keeper.height
        val total = keeper.totalWeight
        
        val defaultRoundVotes = {
            height: height,
            round: vote.round,
            prevotes: newVoteCount(total),
            precommits: newVoteCount(total),
            emittedEvents: Set(),
            votesAddressesWeights: Map()
        }
        val roundVotes = keeper.rounds.getOrElse(vote.round, defaultRoundVotes)
        
        val updatedVoteCount = 
            match vote.typ {
                | Prevote => roundVotes.prevotes.addVote(vote, weight)
                | Precommit => roundVotes.precommits.addVote(vote, weight)
            }

        val updatedVotesAddressesWeights =
            if (roundVotes.votesAddressesWeights.has(vote.address)) 
                roundVotes.votesAddressesWeights
            else 
                roundVotes.votesAddressesWeights.mapSafeSet(vote.address, weight)

        val updatedRoundVotes = 
            match vote.typ {
                | Prevote => roundVotes.with("prevotes", updatedVoteCount)                                   
                | Precommit => roundVotes.with("precommits", updatedVoteCount)
            }

        // Combined weight of all validators at this height
        val combinedWeight = updatedVotesAddressesWeights.mapSumValues()

        val finalEvent =
            if (vote.round > keeper.currentRound and isSkip(combinedWeight, total))
                SkipEvent(vote.round)
            else
                val threshold = computeThreshold(updatedVoteCount, vote.value)
                val event = toEvent(vote.round, vote.typ, threshold)
                if (not(event.in(roundVotes.emittedEvents))) 
                    event
                else 
                    NoEvent(vote.round)

        val updatedEmittedEvents = roundVotes.emittedEvents.setAddIf(finalEvent, not(isNoEvent(finalEvent)))

        val updatedRoundVotes2 = updatedRoundVotes
                .with("votesAddressesWeights", updatedVotesAddressesWeights)
                .with("emittedEvents", updatedEmittedEvents)

        val newBookkeeper =
            keeper.with("rounds", keeper.rounds.mapSafeSet(vote.round, updatedRoundVotes2))
        
        { 
            bookkeeper: newBookkeeper, 
            event: finalEvent 
        }
    
    // Called by the executor to check if there is a specific threshold for a given round and voteType.
    // TO DISCUSS:
    // - The function does not consider Skip threshold. This because if the executor receives a Skip event
    //   and do not act on it, this means that it will never do it in the future. We should discuss that this
    //   is the case.
    pure def checkThreshold(keeper: Bookkeeper, round: Round, voteType: VoteType, threshold: Threshold): bool =
        if (keeper.rounds.has(round)) {
            val roundVotes = keeper.rounds.get(round)
            val voteCount = if (voteType == Prevote) roundVotes.prevotes else roundVotes.precommits
            match threshold {
                | ValueThreshold(v) => voteCount.hasQuorumOnValue(Val(v))
                | NilThreshold => voteCount.hasQuorumOnValue(Nil)
                | AnyThreshold => voteCount.hasQuorumOnAny()
                | SkipThreshold => false
                | UnreachedThreshold => false
            }
        } else false

// run PrecommitTest = all {
// val bin : Bookkeeper = {
//                height: 0,
//                rounds:
//                  Map(
//                    0 ->
//                      {
//                        height: 0,
//                        precommits: { total: 4, valuesAddresses: Map(), valuesWeights: Map() },
//                        prevotes: { total: 4, valuesAddresses: Map(), valuesWeights: Map("a block" -> 1, "nil" -> 3) },
//                        round: 0
//                      }
//                  ),
//                totalWeight: 4
//              }
// val o1 = applyVote(bin, {value: "nil", round: 0, address: "v0", typ: Precommit }, 1)
// val o2 = applyVote(o1.bookkeeper, {value: "nil", round: 0, address: "v1", typ: Precommit }, 1)
// val o3 = applyVote(o2.bookkeeper, {value: "nil", round: 0, address: "v2", typ: Precommit }, 1)
// val o4 = applyVote(o3.bookkeeper, {value: "nil", round: 0, address: "v3", typ: Precommit }, 1)
// all{
//     assert(o1.event.name == "None"),
//     assert(o2.event.name == "None"),
//     assert(o3.event.name == "PrecommitAny"),
//     assert(o4.event.name == "PrecommitAny")
// }
// }

    // ****************************************************************************
    // Unit tests
    // ****************************************************************************

    run isQuorumTest = all {
        assert(isQuorum(0,0) == false),
        assert(isQuorum(2,6) == false),
        assert(isQuorum(4,6) == false),
        assert(isQuorum(5,6) == true)
    }

    run isSkipTest = all {
        assert(isSkip(0,0) == false),
        assert(isSkip(2,6) == false),
        assert(isSkip(3,6) == true)
    }

    run addVoteTest =
        val voteCount = {totalWeight: 100, valuesWeights: Map(Val("v1") -> 30, Val("v2") -> 20), votesAddresses: Set("alice", "bob")}
        val vote = {typ: Precommit, round: 10, value: Val("v3"), address: "john"}
        all {
            // new voter, new value
            assert(addVote(voteCount, vote, 10) == {totalWeight: 100, valuesWeights: Map(Val("v1") -> 30, Val("v2") -> 20, Val("v3") -> 10), votesAddresses: Set("alice", "bob", "john")}),
            // new voter, existing value
            assert(addVote(voteCount, vote.with("value", Val("v2")), 10) == {totalWeight: 100, valuesWeights: Map(Val("v1") -> 30, Val("v2") -> 30), votesAddresses: Set("alice", "bob", "john")}),
            // existing voter
            assert(addVote(voteCount, vote.with("address", "alice"), 10) == voteCount),
        }

    run computeThresholdTest =
        val voteCount = {totalWeight: 100, valuesWeights: Map(), votesAddresses: Set("alice", "bob")}
        val mapValueReached = Map(Val("v1") -> 67, Val("v2") -> 20)
        val mapNilReached = Map(Nil -> 70, Val("v2") -> 20)
        val mapNoneReached = Map(Nil -> 20, Val("v2") -> 20)
        all {
            assert(computeThreshold(voteCount, Val("v3")) == UnreachedThreshold),
            assert(computeThreshold(voteCount.with("valuesWeights", mapValueReached), Val("v1")) == ValueThreshold("v1")),
            assert(computeThreshold(voteCount.with("valuesWeights", mapValueReached), Val("v2")) == AnyThreshold),
            assert(computeThreshold(voteCount.with("valuesWeights", mapNilReached), Nil) == NilThreshold),
            assert(computeThreshold(voteCount.with("valuesWeights", mapNilReached), Val("v2")) == AnyThreshold),
            assert(computeThreshold(voteCount.with("valuesWeights", mapNoneReached), Val("v1")) == UnreachedThreshold),
            assert(computeThreshold(voteCount.with("valuesWeights", mapNoneReached), Nil) == UnreachedThreshold),
        }

    run toEventTest =
        val round = 10
        all {
            assert(toEvent(round, Prevote, UnreachedThreshold) == NoEvent(round)),
            assert(toEvent(round, Precommit, UnreachedThreshold) == NoEvent(round)),
            assert(toEvent(round, Prevote, AnyThreshold) == PolkaAnyEvent(round)),
            assert(toEvent(round, Prevote, NilThreshold) == PolkaNilEvent(round)),
            assert(toEvent(round, Prevote, ValueThreshold("v1")) == PolkaValueEvent((round, Val("v1")))),
            assert(toEvent(round, Precommit, AnyThreshold) == PrecommitAnyEvent(round)),
            assert(toEvent(round, Precommit, NilThreshold) == PrecommitAnyEvent(round)),
            assert(toEvent(round, Precommit, ValueThreshold("v1")) == PrecommitValueEvent((round, Val("v1")))),
            assert(toEvent(round, Prevote, SkipThreshold) == SkipEvent(round)),
            assert(toEvent(round, Precommit, SkipThreshold) == SkipEvent(round)),
        }

}
