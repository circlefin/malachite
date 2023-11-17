// -*- mode: Bluespec; -*-

module voteBookkeeper {

    import basicSpells.* from "./basicSpells"
    import extraSpells.* from "./extraSpells"

    // ****************************************************************************
    // Types
    // ****************************************************************************

    // Node address
    type Address = str

    // Round in the Tendermint algorithm
    type Round = int

    // Height in the Tendermint algorithm
    type Height = int

    // Value proposed and voted to be included in the chain
    type Value = str
    val Nil: Value = "nil" // a special value of type Value

    // The stake of a node
    type Weight = int

    type VoteType = str
    val VoteTypes: Set[VoteType] = Set("Prevote", "Precommit")
    def isPrevote(voteType) = voteType == "Prevote"
    def isPrecommit(voteType) = voteType == "Precommit"

    type Vote = {
        typ: VoteType,
        round: Round,
        value: Value,
        address: Address
    }

    type WeightedVote = (Vote, Weight)

    type VoteCount = {
        totalWeight: Weight,
        valuesWeights: Value -> Weight, // including the "nil" value
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

    type Threshold = {
        name: str,
        value: Value
    }
    val unreachedThreshold         = { name: "Unreached", value: "null" }
    val nilThreshold               = { name: "Nil", value: "null" }
    val anyThreshold               = { name: "Any", value: "null" }
    val skipThreshold              = { name: "Skip", value: "null" }
    pure def valueThreshold(value) = { name: "Value", value: value }
    pure def isThresholdOnValue(t: Threshold): bool = t.name == "Value"

    type ExecutorEvent = {
        round: Round,
        name: str,
        value: Value
    }
    pure def noEvent(round)                    = { round: round, name: "None", value: "null" }
    pure def polkaValueEvent(round, value)     = { round: round, name: "PolkaValue", value: value }
    pure def polkaNilEvent(round)              = { round: round, name: "PolkaNil", value: "null" }
    pure def polkaAnyEvent(round)              = { round: round, name: "PolkaAny", value: "null" }
    pure def precommitValueEvent(round, value) = { round: round, name: "PrecommitValue", value: value }
    pure def precommitAnyEvent(round)          = { round: round, name: "PrecommitAny", value: "null" }
    pure def skipEvent(round)                  = { round: round, name: "Skip", value: "null" }

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

    pure def newRoundVotes(height: Height, round: Round, totalWeight: Weight): RoundVotes = {
        height: height,
        round: round,
        prevotes: newVoteCount(totalWeight),
        precommits: newVoteCount(totalWeight),
        emittedEvents: Set(),
        votesAddressesWeights: Map()
    }

    // creates a new voteCount
    pure def newVoteCount(total: Weight): VoteCount =
        { totalWeight: total, valuesWeights: Map(), votesAddresses: Set() }

    // Returns true if weight > 2/3 * total (quorum: at least f+1 correct)
    pure def isQuorum(weight: Weight, total: Weight): bool =
        3 * weight > 2 * total

    run isQuorumTest = all {
        assert(isQuorum(0,0) == false),
        assert(isQuorum(2,6) == false),
        assert(isQuorum(4,6) == false),
        assert(isQuorum(5,6) == true),
    }

    // True iff the vote count has a quorum on a specific value.
    pure def hasQuorumOnValue(voteCount: VoteCount, value: Value): bool =
        isQuorum(voteCount.valuesWeights.getOrElse(value, 0), voteCount.totalWeight)

    // True iff the vote count has a quorum on value nil.
    pure def hasQuorumOnNil(voteCount: VoteCount): bool =
        hasQuorumOnValue(voteCount, Nil)

    // True iff the vote count has a quorum on any value.
    pure def hasQuorumOnAny(voteCount: VoteCount): bool =
        isQuorum(voteCount.valuesWeights.mapSumValues(), voteCount.totalWeight)

    // Returns true if weight > 1/3 * total (small quorum: at least one correct)
    pure def isSkip(weight: Weight, total: Weight): bool =
        3 * weight > total

    run isSkipTest = all {
        assert(isSkip(0,0) == false),
        assert(isSkip(2,6) == false),
        assert(isSkip(3,6) == true),
    }

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

    run addVoteTest =
        val voteCount = { 
            totalWeight: 100, 
            valuesWeights: Map("val1" -> 30, "val2" -> 20), 
            votesAddresses: Set("alice", "bob")
        }
        val vote = { typ: "Precommit", round: 10, value: "val3", address: "john" }
        all {
            // new voter, new value
            assert(addVote(voteCount, vote, 10) == { 
                totalWeight: 100, 
                valuesWeights: Map("val1" -> 30, "val2" -> 20, "val3" -> 10), 
                votesAddresses: Set("alice", "bob", "john") 
            }),
            // new voter, existing value
            assert(addVote(voteCount, vote.with("value", "val2"), 10) == { 
                totalWeight: 100, 
                valuesWeights: Map("val1" -> 30, "val2" -> 30), 
                votesAddresses: Set("alice", "bob", "john") 
            }),
            // existing voter
            assert(addVote(voteCount, vote.with("address", "alice"), 10) == voteCount),
        }

    // Given a voteCount and a value, the function returns:
    // - A threshold Value if there is a quorum for the given value;
    // - A threshold Nil if there is a quorum for the nil and no quorum for the value;
    // - A threshold Any if there is no quorum for the value or nil and there is a quroum for any (including nil);
    // - A threshold Unreached otherwise indicating that no quorum has been yet reached.
    pure def computeThreshold(voteCount: VoteCount, value: Value): Threshold =
        if (voteCount.hasQuorumOnValue(value)) {
            if (value == Nil) 
                nilThreshold
            else 
                valueThreshold(value)
        } else if (voteCount.hasQuorumOnAny()) {
            anyThreshold
        } else 
            unreachedThreshold

    // Given a round, voteType and threshold, return the corresponding ExecutorEvent
    pure def toEvent(round: Round, voteType: VoteType, threshold: Threshold): ExecutorEvent =
        if (threshold == unreachedThreshold) 
            noEvent(round)
        
        // prevote
        else if (voteType.isPrevote() and threshold.isThresholdOnValue()) 
            polkaValueEvent(round, threshold.value)
        else if (voteType.isPrevote() and threshold == nilThreshold) 
            polkaNilEvent(round)
        else if (voteType.isPrevote() and threshold == anyThreshold) 
            polkaAnyEvent(round)
        
        // precommit
        else if (voteType.isPrecommit() and threshold.isThresholdOnValue()) 
            precommitValueEvent(round, threshold.value)
        else if (voteType.isPrecommit() and threshold.in(Set(anyThreshold, nilThreshold)))
            precommitAnyEvent(round)

        else if (threshold == skipThreshold)
            skipEvent(round)
        
        else 
            noEvent(round)

    run toEventTest =
        val round = 10
        all {
            assert(toEvent(round, "Prevote", unreachedThreshold) == noEvent(round)),
            assert(toEvent(round, "Precommit", unreachedThreshold) == noEvent(round)),
            
            assert(toEvent(round, "Prevote", anyThreshold) == polkaAnyEvent(round)),
            assert(toEvent(round, "Prevote", nilThreshold) == polkaNilEvent(round)),
            assert(toEvent(round, "Prevote", valueThreshold("val1")) == polkaValueEvent(round, "val1")),
            
            assert(toEvent(round, "Precommit", anyThreshold) == precommitAnyEvent(round)),
            assert(toEvent(round, "Precommit", nilThreshold) == precommitAnyEvent(round)),
            assert(toEvent(round, "Precommit", valueThreshold("val1")) == precommitValueEvent(round, "val1")),
            
            assert(toEvent(round, "Prevote", skipThreshold) == skipEvent(round)),
            assert(toEvent(round, "Precommit", skipThreshold) == skipEvent(round)),
            
            assert(toEvent(round, "Precommit", { name: "Error", value: "null" }) == noEvent(round)),
            assert(toEvent(round, "Error", anyThreshold) == noEvent(round)),
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
        val round = vote.round
        val roundVotes = keeper.rounds.getOrElse(round, newRoundVotes(keeper.height, round, keeper.totalWeight))
        
        val updatedVoteCount = 
            if (vote.typ.isPrevote()) 
                roundVotes.prevotes.addVote(vote, weight)
            else 
                roundVotes.precommits.addVote(vote, weight)

        // CHECK: if alice first prevotes with weight 10 and then alice precommits with weight 60, only 10 is registered here.
        val updatedVotesAddressesWeights =
            if (roundVotes.votesAddressesWeights.has(vote.address)) 
                roundVotes.votesAddressesWeights
            else 
                roundVotes.votesAddressesWeights.mapSafeSet(vote.address, weight)

        val updatedRoundVotes = 
            if (vote.typ.isPrevote()) 
                roundVotes.with("prevotes", updatedVoteCount)                                   
            else 
                roundVotes.with("precommits", updatedVoteCount)

        // Combined weight of all validators at this height
        val combinedWeight = updatedVotesAddressesWeights.mapSumValues()

        val finalEvent =
            if (vote.round > keeper.currentRound and isSkip(combinedWeight, keeper.totalWeight))
                skipEvent(vote.round)
            else
                val threshold = computeThreshold(updatedVoteCount, vote.value)
                val event = toEvent(vote.round, vote.typ, threshold)
                if (not(event.in(roundVotes.emittedEvents))) 
                    event
                else 
                    noEvent(vote.round)

        val updatedEmittedEvents = roundVotes.emittedEvents.setAddIf(finalEvent, finalEvent.name != "None")

        val updatedRoundVotes2 = updatedRoundVotes
                .with("votesAddressesWeights", updatedVotesAddressesWeights)
                .with("emittedEvents", updatedEmittedEvents)

        val newBookkeeper =
            keeper.with("rounds", keeper.rounds.mapSafeSet(vote.round, updatedRoundVotes2))

        { 
            bookkeeper: newBookkeeper, 
            event: finalEvent 
        }

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
    // val o1 = applyVote(bin, {value: "nil", round: 0, address: "v0", typ: "Precommit" }, 1)
    // val o2 = applyVote(o1.bookkeeper, {value: "nil", round: 0, address: "v1", typ: "Precommit" }, 1)
    // val o3 = applyVote(o2.bookkeeper, {value: "nil", round: 0, address: "v2", typ: "Precommit" }, 1)
    // val o4 = applyVote(o3.bookkeeper, {value: "nil", round: 0, address: "v3", typ: "Precommit" }, 1)
    // all{
    //     assert(o1.event.name == "None"),
    //     assert(o2.event.name == "None"),
    //     assert(o3.event.name == "PrecommitAny"),
    //     assert(o4.event.name == "PrecommitAny")
    // }
    // }

    // Called by the executor to check if there is a specific threshold for a given round and voteType.
    // TO DISCUSS:
    // - The function does not consider Skip threshold. This because if the executor receives a Skip event
    //   and do not act on it, this means that it will never do it in the future. We should discuss that this
    //   is the case.
    pure def checkThreshold(keeper: Bookkeeper, round: Round, voteType: VoteType, threshold: Threshold): bool =
        if (keeper.rounds.has(round)) {
            val roundVotes = keeper.rounds.get(round)
            val voteCount = if (voteType.isPrevote()) roundVotes.prevotes else roundVotes.precommits
            checkThresholdOnVoteCount(voteCount, threshold)
        } else false

    pure def checkThresholdOnVoteCount(voteCount: VoteCount, threshold: Threshold): bool =
        if (threshold.isThresholdOnValue())
            voteCount.hasQuorumOnValue(threshold.value)
        else if (threshold == nilThreshold)
            voteCount.hasQuorumOnNil()
        else if (threshold == anyThreshold)
            voteCount.hasQuorumOnAny()
        else false

    run computeThresholdTest =
        val voteCount = { totalWeight: 100, valuesWeights: Map(), votesAddresses: Set("alice", "bob") }
        val mapValueReached = Map("val1" -> 67, "val2" -> 20)
        val mapNilReached = Map(Nil -> 70, "val2" -> 20)
        val mapNoneReached = Map(Nil -> 20, "val2" -> 20)
        all {
            assert(computeThreshold(voteCount, "val3") == unreachedThreshold),
            assert(computeThreshold(voteCount.with("valuesWeights", mapValueReached), "val1") == valueThreshold("val1")),
            assert(computeThreshold(voteCount.with("valuesWeights", mapValueReached), "val2") == anyThreshold),
            assert(computeThreshold(voteCount.with("valuesWeights", mapNilReached), Nil) == nilThreshold),
            assert(computeThreshold(voteCount.with("valuesWeights", mapNilReached), "val2") == anyThreshold),
            assert(computeThreshold(voteCount.with("valuesWeights", mapNoneReached), "val1") == unreachedThreshold),
            assert(computeThreshold(voteCount.with("valuesWeights", mapNoneReached), Nil) == unreachedThreshold),
        }

    // ************************************************************************
    // Properties/Invariants
    // ************************************************************************

    // Each weight in a voteCount is less or equal than the total weight.
    def voteValidWeightInv(voteCount) = 
        voteCount.valuesWeights.forallValues(weight => weight <= voteCount.totalWeight)

    // The sum of all weights is less or equal than the total weight.
    def voteValidWeightSumInv(voteCount: VoteCount): bool = 
        voteCount.valuesWeights.mapSumValues() <= voteCount.totalWeight

    def roundVotesInv(rounds: Round -> RoundVotes): bool = 
        rounds.forallEntries((round, roundVotes) => all {
            voteValidWeightInv(roundVotes.prevotes),
            voteValidWeightInv(roundVotes.precommits),
            voteValidWeightSumInv(roundVotes.prevotes),
            voteValidWeightSumInv(roundVotes.precommits),
        })

    def Inv = all {
        roundVotesInv(bookkeeper.rounds)
    }

    // ************************************************************************
    // State 
    // ************************************************************************

    // The value used as parameter on each action taken.
    var weightedVote: WeightedVote
    // The state of the Bookkeeper.
    var bookkeeper: Bookkeeper
    // The event resulting from applying a weighted vote to the bookkeeper.
    var lastEmitted: ExecutorEvent

    // ************************************************************************
    // Actions
    // ************************************************************************

    action allUnchanged: bool = all {
        weightedVote' = weightedVote,
        bookkeeper' = bookkeeper,
        lastEmitted' = lastEmitted,
    }

    action initWith(initialHeight: Height, initialRound: Round, totalWeight: Weight): bool = all {
        weightedVote' = ({ typ: "", round: -1, value: "", address: "" }, -1),
        bookkeeper' = { height: initialHeight, currentRound: initialRound, totalWeight: totalWeight, rounds: Map() },
        lastEmitted' = { round: -1, name: "", value: "null" }
    }

    // The vote keeper receives a weighted vote and produces an event.
    action applyVoteAction(vote: Vote, weight: Weight): bool =
        val result = applyVote(bookkeeper, vote, weight)
        all {
            weightedVote' = (vote, weight),
            bookkeeper' = result.bookkeeper,
            lastEmitted' = result.event
        }

}
