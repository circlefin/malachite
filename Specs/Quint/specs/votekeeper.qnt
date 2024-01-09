// -*- mode: Bluespec; -*-

module voteBookkeeper {

    import types.* from "./types"
    import basicSpells.* from "./basicSpells"
    import extraSpells.* from "./extraSpells"

    // ****************************************************************************
    // Types
    // ****************************************************************************

    type WeightedVote = 
        | NoWeightedVote 
        | WV((Vote, Weight, Round))

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
        emittedOutputs: Set[VoteKeeperOutput],
        votesAddressesWeights: Address -> Weight
    }

    type Threshold =
        | UnreachedThreshold
        | NilThreshold
        | AnyThreshold
        | SkipThreshold
        | ValueThreshold(NonNilValue)

    type VoteKeeperOutput = 
        | NoVKOutput
        | PolkaAnyVKOutput(Round)
        | PolkaNilVKOutput(Round)
        | PolkaValueVKOutput((Round, NonNilValue))
        | PrecommitAnyVKOutput(Round)
        | PrecommitValueVKOutput((Round, NonNilValue))
        | SkipVKOutput(Round)

    type Bookkeeper = {
        height: Height,
        totalWeight: Weight,
        rounds: Round -> RoundVotes
    }

    pure def initBookKeeper(initialHeight: Height, totalVotingPower: Weight): Bookkeeper = { 
        height: initialHeight, 
        totalWeight: totalVotingPower, 
        rounds: Map()
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
        if (vote.srcAddress.in(voteCount.votesAddresses))
            // Do not count vote if address has already voted.
            voteCount
        else 
            val newWeight = voteCount.valuesWeights.getOrElse(vote.valueId, 0) + weight             
            voteCount
                .with("valuesWeights", voteCount.valuesWeights.mapSafeSet(vote.valueId, newWeight))
                .with("votesAddresses", voteCount.votesAddresses.setAdd(vote.srcAddress))

    run addVoteTest =
        val voteCount = { 
            totalWeight: 100, 
            valuesWeights: Map(Val("val1") -> 30, Val("val2") -> 20), 
            votesAddresses: Set("alice", "bob")
        }
        val vote = mkVote(Precommit, "john", 1, 10, Val("val3"))
        all {
            // new voter, new value
            assert(addVote(voteCount, vote, 10) == { 
                totalWeight: 100, 
                valuesWeights: Map(Val("val1") -> 30, Val("val2") -> 20, Val("val3") -> 10), 
                votesAddresses: Set("alice", "bob", "john") 
            }),
            // new voter, existing value
            assert(addVote(voteCount, vote.with("valueId", Val("val2")), 10) == { 
                totalWeight: 100, 
                valuesWeights: Map(Val("val1") -> 30, Val("val2") -> 30), 
                votesAddresses: Set("alice", "bob", "john") 
            }),
            // existing voter
            assert(addVote(voteCount, vote.with("srcAddress", "alice"), 10) == voteCount),
        }

    // Given a voteCount and a value, the function returns:
    // - A threshold Value if there is a quorum for the given value;
    // - A threshold Nil if there is a quorum for the nil and no quorum for the value;
    // - A threshold Any if there is no quorum for the value or nil and there is a quorum for any (including nil);
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

    // Given a round, voteType and threshold, return the corresponding VoteKeeperOutput
    pure def toVoteKeeperOutput(round: Round, voteType: VoteType, threshold: Threshold): VoteKeeperOutput =
        match threshold {
            | UnreachedThreshold => 
                NoVKOutput
            | ValueThreshold(value) =>
                match voteType {
                    | Prevote => PolkaValueVKOutput((round, value))
                    | Precommit => PrecommitValueVKOutput((round, value))
                }
            | NilThreshold =>
                match voteType {
                    | Prevote => PolkaNilVKOutput(round)
                    | Precommit => PrecommitAnyVKOutput(round)
                }
            | AnyThreshold =>
                match voteType {
                    | Prevote => PolkaAnyVKOutput(round)
                    | Precommit => PrecommitAnyVKOutput(round)
                }
            | SkipThreshold => 
                SkipVKOutput(round)
        }

    run toVoteKeeperOutputTest =
        val round = 10
        all {
            assert(toVoteKeeperOutput(round, Prevote, UnreachedThreshold) == NoVKOutput),
            assert(toVoteKeeperOutput(round, Precommit, UnreachedThreshold) == NoVKOutput),
            assert(toVoteKeeperOutput(round, Prevote, AnyThreshold) == PolkaAnyVKOutput(round)),
            assert(toVoteKeeperOutput(round, Prevote, NilThreshold) == PolkaNilVKOutput(round)),
            assert(toVoteKeeperOutput(round, Prevote, ValueThreshold("v1")) == PolkaValueVKOutput((round, "v1"))),
            assert(toVoteKeeperOutput(round, Precommit, AnyThreshold) == PrecommitAnyVKOutput(round)),
            assert(toVoteKeeperOutput(round, Precommit, NilThreshold) == PrecommitAnyVKOutput(round)),
            assert(toVoteKeeperOutput(round, Precommit, ValueThreshold("v1")) == PrecommitValueVKOutput((round, "v1"))),
            assert(toVoteKeeperOutput(round, Prevote, SkipThreshold) == SkipVKOutput(round)),
            assert(toVoteKeeperOutput(round, Precommit, SkipThreshold) == SkipVKOutput(round)),
        }

    // Called by the executor when it receives a vote. The function takes the following steps:
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
            match vote.voteType {
                | Prevote => roundVotes.prevotes.addVote(vote, weight)
                | Precommit => roundVotes.precommits.addVote(vote, weight)
            }

        val updatedVotesAddressesWeights =
            if (roundVotes.votesAddressesWeights.has(vote.srcAddress)) 
                roundVotes.votesAddressesWeights
            else 
                roundVotes.votesAddressesWeights.mapSafeSet(vote.srcAddress, weight)

        // Combined weight of all validators at this height
        val combinedWeight = updatedVotesAddressesWeights.mapSumValues()

        val finalOutput =
            if (vote.round > currentRound and isSkip(combinedWeight, keeper.totalWeight))
                SkipVKOutput(vote.round)
            else
                val threshold = computeThreshold(updatedVoteCount, vote.valueId)
                val output = toVoteKeeperOutput(vote.round, vote.voteType, threshold)
                if (not(output.in(roundVotes.emittedOutputs))) 
                    output
                else 
                    NoVKOutput

        val updatedRoundVotes = 
            match vote.voteType {
                | Prevote => roundVotes.with("prevotes", updatedVoteCount)                                   
                | Precommit => roundVotes.with("precommits", updatedVoteCount)
            }
        val updatedRoundVotes2 = updatedRoundVotes
            .with("votesAddressesWeights", updatedVotesAddressesWeights)
            .with("emittedOutputs", roundVotes.emittedOutputs.setAddIf(finalOutput, finalOutput != NoVKOutput))

        val updatedBookkeeper = keeper
            .with("rounds", keeper.rounds.mapSafeSet(vote.round, updatedRoundVotes2))

        { bookkeeper: updatedBookkeeper, output: finalOutput }

    run applyVoteTest =
        val roundVotes: RoundVotes = {
            height: 0,
            round: 0,
            prevotes: { totalWeight: 4, votesAddresses: Set(), valuesWeights: Map(Val("v1") -> 1, Nil -> 3) },
            precommits: { totalWeight: 4, votesAddresses: Set(), valuesWeights: Map() },
            emittedOutputs: Set(),
            votesAddressesWeights: Map(),
        }
        val vk: Bookkeeper = { height: 0, totalWeight: 4, rounds: Map(0 -> roundVotes) }
        val o1 = applyVote(vk, mkVote(Precommit, "a0", 0, 0, Nil), 1, 0)
        val o2 = applyVote(o1.bookkeeper, mkVote(Precommit, "a1", 0, 0, Nil), 1, 0)
        val o3 = applyVote(o2.bookkeeper, mkVote(Precommit, "a2", 0, 0, Nil), 1, 0)
        val o4 = applyVote(o3.bookkeeper, mkVote(Precommit, "a3", 0, 0, Nil), 1, 0)
        val o5 = applyVote(o4.bookkeeper, mkVote(Precommit, "a4", 0, 1, Nil), 3, 0)
        all {
            assert(o1.output == NoVKOutput),
            assert(o2.output == NoVKOutput),
            assert(o3.output == PrecommitAnyVKOutput(0)),
            assert(o4.output == NoVKOutput),
            assert(o5.output == SkipVKOutput(1)),
        }

    // Called by the driver to check if there is a specific threshold for a given round and voteType.
    // TO DISCUSS:
    // - The function does not consider Skip threshold. This because if the driver receives a Skip output
    //   and do not act on it, this means that it will never do it in the future. We should discuss that this
    //   is the case.
    pure def checkThreshold(keeper: Bookkeeper, round: Round, voteType: VoteType, threshold: Threshold): bool =
        if (keeper.rounds.has(round)) {
            val roundVotes = keeper.rounds.get(round)
            val voteCount = if (voteType == Prevote) roundVotes.prevotes else roundVotes.precommits
            checkThresholdOnVoteCount(threshold, voteCount)
        } else false

    pure def checkThresholdOnVoteCount(threshold: Threshold, voteCount: VoteCount): bool =
        match threshold {
            | ValueThreshold(v) => voteCount.hasQuorumOnValue(Val(v))
            | NilThreshold => voteCount.hasQuorumOnValue(Nil)
            | AnyThreshold => voteCount.hasQuorumOnAny()
            | _ => false
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
    // The output outputting from applying a weighted vote to the bookkeeper.
    var lastEmitted: VoteKeeperOutput

    // ************************************************************************
    // Actions
    // ************************************************************************

    action allUnchanged: bool = all {
        weightedVote' = weightedVote,
        bookkeeper' = bookkeeper,
        lastEmitted' = lastEmitted,
    }

    action initWith(initialHeight: Height, initialTotalWeight: Weight): bool = all {
        weightedVote' = NoWeightedVote,
        bookkeeper' = initBookKeeper(initialHeight, initialTotalWeight),
        lastEmitted' = NoVKOutput,
    }

    // The vote bookkeeper receives a weighted vote for a round, and produces an output.
    action applyVoteAction(vote: Vote, weight: Weight, currentRound: Round): bool =
        // HV: currentRound should be taken from the bookkeeper?
        val output = applyVote(bookkeeper, vote, weight, currentRound)
        all {
            weightedVote' = WV((vote, weight, currentRound)),
            bookkeeper' = output.bookkeeper,
            lastEmitted' = output.output
        }

}
