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
        height: Height,
        round: Round,
        value: Value,
        address: Address
    }

    type WeightedVote = (Vote, Weight, Round)

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
        emittedOutputs: Set[VoteKeeperOutput],
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

    type VoteKeeperOutput = {
        round: Round,
        name: str,
        value: Value
    }
    pure def noOutput(round)                    = { round: round, name: "None", value: "null" }
    pure def polkaValueOutput(round, value)     = { round: round, name: "PolkaValue", value: value }
    pure def polkaNilOutput(round)              = { round: round, name: "PolkaNil", value: "null" }
    pure def polkaAnyOutput(round)              = { round: round, name: "PolkaAny", value: "null" }
    pure def precommitValueOutput(round, value) = { round: round, name: "PrecommitValue", value: value }
    pure def precommitAnyOutput(round)          = { round: round, name: "PrecommitAny", value: "null" }
    pure def skipOutput(round)                  = { round: round, name: "Skip", value: "null" }

    type Bookkeeper = {
        height: Height,
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
        emittedOutputs: Set(),
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

    // Adds a weighted vote to a voteCount if there is no vote registered for the voter.
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
        val vote = { typ: "Precommit", height: 1, round: 10, value: "val3", address: "john" }
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
    // - A threshold Any if there is no quorum for the value or nil and there is a quorum for any (including nil);
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

    // Given a round, voteType and threshold, return the corresponding VoteKeeperOutput
    pure def toVoteKeeperOutput(round: Round, voteType: VoteType, threshold: Threshold): VoteKeeperOutput =
        if (threshold == unreachedThreshold) 
            noOutput(round)
        
        // prevote
        else if (voteType.isPrevote() and threshold.isThresholdOnValue()) 
            polkaValueOutput(round, threshold.value)
        else if (voteType.isPrevote() and threshold == nilThreshold) 
            polkaNilOutput(round)
        else if (voteType.isPrevote() and threshold == anyThreshold) 
            polkaAnyOutput(round)
        
        // precommit
        else if (voteType.isPrecommit() and threshold.isThresholdOnValue()) 
            precommitValueOutput(round, threshold.value)
        else if (voteType.isPrecommit() and threshold.in(Set(anyThreshold, nilThreshold)))
            precommitAnyOutput(round)

        else if (threshold == skipThreshold)
            skipOutput(round)
        
        else 
            noOutput(round)

    run toVoteKeeperOutputTest =
        val round = 10
        all {
            assert(toVoteKeeperOutput(round, "Prevote", unreachedThreshold) == noOutput(round)),
            assert(toVoteKeeperOutput(round, "Precommit", unreachedThreshold) == noOutput(round)),
            
            assert(toVoteKeeperOutput(round, "Prevote", anyThreshold) == polkaAnyOutput(round)),
            assert(toVoteKeeperOutput(round, "Prevote", nilThreshold) == polkaNilOutput(round)),
            assert(toVoteKeeperOutput(round, "Prevote", valueThreshold("val1")) == polkaValueOutput(round, "val1")),
            
            assert(toVoteKeeperOutput(round, "Precommit", anyThreshold) == precommitAnyOutput(round)),
            assert(toVoteKeeperOutput(round, "Precommit", nilThreshold) == precommitAnyOutput(round)),
            assert(toVoteKeeperOutput(round, "Precommit", valueThreshold("val1")) == precommitValueOutput(round, "val1")),
            
            assert(toVoteKeeperOutput(round, "Prevote", skipThreshold) == skipOutput(round)),
            assert(toVoteKeeperOutput(round, "Precommit", skipThreshold) == skipOutput(round)),
            
            assert(toVoteKeeperOutput(round, "Precommit", { name: "Error", value: "null" }) == noOutput(round)),
            assert(toVoteKeeperOutput(round, "Error", anyThreshold) == noOutput(round)),
        }

    // Driver interface
    type BKOutput = { bookkeeper: Bookkeeper, output: VoteKeeperOutput }

    // Called by the driver when it receives a vote. The function takes the following steps:
    // - It first adds the vote and then computes a threshold.
    // - If there exist a threshold and has not emitted before, the function returns the corresponding VoteKeeperOutput.
    // - Otherwise, the function returns a no-threshold output.
    // - Note that if there is no threshold after adding the vote, the function checks if there is a skip threshold.
    // TO DISCUSS:
    // - There might be a problem if we generalize from single-shot to multi-shot: the keeper only keeps the totalWeight
    //   of the current height; I wonder if we need to keep the totalWeight for every Height that we may receive a vote for.
    pure def applyVote(keeper: Bookkeeper, vote: Vote, weight: Weight, currentRound: Round): { bookkeeper: Bookkeeper, output: VoteKeeperOutput } =
        val round = vote.round
        val roundVotes = keeper.rounds.getOrElse(round, newRoundVotes(keeper.height, round, keeper.totalWeight))
        
        val updatedVoteCount = 
            if (vote.typ.isPrevote()) 
                roundVotes.prevotes.addVote(vote, weight)
            else 
                roundVotes.precommits.addVote(vote, weight)

        val updatedVotesAddressesWeights =
            if (roundVotes.votesAddressesWeights.has(vote.address)) 
                roundVotes.votesAddressesWeights
            else 
                roundVotes.votesAddressesWeights.mapSafeSet(vote.address, weight)

        // Combined weight of all validators at this height
        val combinedWeight = updatedVotesAddressesWeights.mapSumValues()

        val finalOutput =
            if (vote.round > currentRound and isSkip(combinedWeight, keeper.totalWeight))
                skipOutput(vote.round)
            else
                val threshold = computeThreshold(updatedVoteCount, vote.value)
                val output = toVoteKeeperOutput(vote.round, vote.typ, threshold)
                if (not(output.in(roundVotes.emittedOutputs))) 
                    output
                else 
                    noOutput(vote.round)

        val updatedRoundVotes = 
            if (vote.typ.isPrevote()) 
                roundVotes.with("prevotes", updatedVoteCount)                                   
            else
                roundVotes.with("precommits", updatedVoteCount)
        val updatedRoundVotes2 = updatedRoundVotes
            .with("votesAddressesWeights", updatedVotesAddressesWeights)
            .with("emittedOutputs", roundVotes.emittedOutputs.setAddIf(finalOutput, finalOutput.name != "None"))

        val updatedBookkeeper = keeper
            .with("rounds", keeper.rounds.mapSafeSet(vote.round, updatedRoundVotes2))

        { bookkeeper: updatedBookkeeper, output: finalOutput }

    run applyVoteTest =
        val roundVotes: RoundVotes = {
            height: 0,
            round: 0,
            prevotes: { totalWeight: 4, votesAddresses: Set(), valuesWeights: Map("v1" -> 1, Nil -> 3) },
            precommits: { totalWeight: 4, votesAddresses: Set(), valuesWeights: Map() },
            emittedOutputs: Set(),
            votesAddressesWeights: Map(),
        }
        val vk: Bookkeeper = { height: 0, totalWeight: 4, rounds: Map(0 -> roundVotes) }
        val o1 = applyVote(vk, { height: 0, round: 0, address: "a0", typ: "Precommit", value: Nil }, 1, 0)
        val o2 = applyVote(o1.bookkeeper, { height: 0, round: 0, address: "a1", typ: "Precommit", value: Nil }, 1, 0)
        val o3 = applyVote(o2.bookkeeper, { height: 0, round: 0, address: "a2", typ: "Precommit", value: Nil }, 1, 0)
        val o4 = applyVote(o3.bookkeeper, { height: 0, round: 0, address: "a3", typ: "Precommit", value: Nil }, 1, 0)
        val o5 = applyVote(o4.bookkeeper, { height: 0, round: 1, address: "a4", typ: "Precommit", value: Nil }, 3, 0)
        all {
            assert(o1.output.name == "None"),
            assert(o2.output.name == "None"),
            assert(o3.output.name == "PrecommitAny"),
            assert(o4.output.name == "None"),
            assert(o5.output.name == "Skip"),
        }

    // Called by the driver to check if there is a specific threshold for a given round and voteType.
    // TO DISCUSS:
    // - The function does not consider Skip threshold. This because if the driver receives a Skip output
    //   and do not act on it, this means that it will never do it in the future. We should discuss that this
    //   is the case.
    pure def checkThreshold(keeper: Bookkeeper, round: Round, voteType: VoteType, threshold: Threshold): bool =
        if (keeper.rounds.has(round)) {
            val roundVotes = keeper.rounds.get(round)
            val voteCount = if (voteType.isPrevote()) roundVotes.prevotes else roundVotes.precommits
            checkThresholdOnVoteCount(threshold, voteCount)
        } else false

    pure def checkThresholdOnVoteCount(threshold: Threshold, voteCount: VoteCount): bool =
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
    // The output outputing from applying a weighted vote to the bookkeeper.
    var lastEmitted: VoteKeeperOutput

    // ************************************************************************
    // Actions
    // ************************************************************************

    action allUnchanged: bool = all {
        weightedVote' = weightedVote,
        bookkeeper' = bookkeeper,
        lastEmitted' = lastEmitted,
    }

    action initWith(initialHeight: Height, totalWeight: Weight): bool = all {
        weightedVote' = ({ typ: "", height: initialHeight, round: -1, value: "", address: "" }, -1, -1),
        bookkeeper' = { height: initialHeight, totalWeight: totalWeight, rounds: Map() },
        lastEmitted' = { round: -1, name: "", value: "null" }
    }

    // The vote keeper receives a weighted vote and produces an output.
    action applyVoteAction(vote: Vote, weight: Weight, currentRound: Round): bool =
        val output = applyVote(bookkeeper, vote, weight, currentRound)
        all {
            weightedVote' = (vote, weight, currentRound),
            bookkeeper' = output.bookkeeper,
            lastEmitted' = output.output
        }

}
