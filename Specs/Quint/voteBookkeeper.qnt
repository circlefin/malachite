// -*- mode: Bluespec; -*-

module voteBookkeeper {

    import basicSpells.* from "./basicSpells"
    import extraSpells.* from "./extraSpells"

    // An address is an string
    type Address = str

    // A round is an integer
    type Round = int

    // A height is an integer
    type Height = int

    // A value is a string
    type Value = str

    // Weigth is an integer
    type Weight = int

    type ValueWeights = Value -> Weight

    // The vote type
    type Vote = {
        typ: str,
        round: Round,
        value: Value,
        address: Address
    }

    type VoteCount = {
        total: Weight,
        // includes nil
        valuesWeights: ValueWeights,
        votesAddresses: Set[Address]
    }

    type RoundVotes = {
        height: Height,
        round: Round,
        prevotes: VoteCount,
        precommits: VoteCount,
        emittedThresholds: Set[Threshold],
        votesAddressesWeights: Address -> Weight
    }

    type Threshold = {
        name: str,
        value: Value
    }

    type ExecutorEvent = {
        round: Round,
        name: str,
        value: Value
    }

    type Bookkeeper = {
        height: Height,
        totalWeight: Weight,
        rounds: Round -> RoundVotes
    }

    // Internal functions

    pure def newVoteCount(total: Weight): VoteCount = {
        {total: total, valuesWeights: Map(), votesAddresses: Set()}
    }

    pure def isQuorum(weight: int, total: int): bool = {
        3 * weight > 2 * total
    }

    pure def isSkip(weight: int, total: int): bool = {
        3 * weight > total
    }

    pure def addVote(voteCount: VoteCount, vote: Vote, weight: Weight): VoteCount = {
        //val total = voteCount.total
        if (vote.address.in(voteCount.votesAddresses)) voteCount
        else val newWeight = voteCount.valuesWeights.getOrElse(vote.value, 0) + weight             
             voteCount.with("valuesWeights", voteCount.valuesWeights.mapSafeSet(vote.value, newWeight))
    }

    pure def computeThreshold(voteCount: VoteCount, value: Value, totalWeight: Weight): Threshold = {
        val weight = voteCount.valuesWeights.get(value)
        val sumWeight = voteCount.valuesWeights.keys().fold(0, (sum, v) => sum + voteCount.valuesWeights.get(v))
        if (value != "nil" and isQuorum(weight, totalWeight)) {name: "Value", value: value}
        else if (value == "nil" and isQuorum(weight, totalWeight)) {name: "Nil", value: "null"}
        else if (isQuorum(sumWeight, totalWeight)) {name: "Any", value: "null"}
        else {name: "Unreached", value: "null"}
    }

    pure def toEvent(round: Round, voteType: str, threshold: Threshold): ExecutorEvent = {
        if (threshold.name == "Unreached") {round: round, name: "None", value: "null"}
        else if (voteType == "Prevote" and threshold.name == "Value") {round: round, name: "PolkaValue", value: threshold.value}
        else if (voteType == "Prevote" and threshold.name == "Nil") {round: round, name: "PolkaNil", value: "null"}
        else if (voteType == "Prevote" and threshold.name == "Any") {round: round, name: "PolkaAny", value: "null"}
        else if (voteType == "Precommit" and threshold.name == "Value") {round: round, name: "PrecommitValue", value: threshold.value}
        else if (voteType == "Precommit" and threshold.name == "Any") {round: round, name: "PrecommitAny", value: "null"}
        else {round: round, name: "None", value: "null"}
    }

    // Executor interface

    pure def applyVote(keeper: Bookkeeper, vote: Vote, weight: int): {bookkeper: Bookkeeper, event: ExecutorEvent} = {
        val height = keeper.height
        val total = keeper.totalWeight
        val roundVotes = keeper.rounds.getOrElse(vote.round, {height: height,
                                                              round: vote.round,
                                                              prevotes: newVoteCount(total),
                                                              precommits: newVoteCount(total),
                                                              emittedThresholds: Set(),
                                                              votesAddressesWeights: Map()})
        val updatedVoteCount = if (vote.typ == "Prevote") roundVotes.prevotes.addVote(vote, weight)
                               else roundVotes.precommits.addVote(vote, weight)
        val threshold = computeThreshold(updatedVoteCount, vote.value, total)
        val updatedVotesAddressesWeights = if (roundVotes.votesAddressesWeights.has(vote.address)) roundVotes.votesAddressesWeights
                                           else roundVotes.votesAddressesWeights.mapSafeSet(vote.address, weight)
        val sumSkip = updatedVotesAddressesWeights.keys().fold(0, (sum, v) => sum + updatedVotesAddressesWeights.get(v))
        val skipThreshold = {name: "Skip", value: "null"}
        val finalThreshold = if (not(threshold.in(roundVotes.emittedThresholds))) threshold
                             else if (isSkip(sumSkip, total) and not(skipThreshold.in(roundVotes.emittedThresholds))) {name: "Skip", value: "null"}
                             else {name: "Unreached", value: "null"}

        val updatedRoundVotes = if (vote.typ == "Prevote") roundVotes.with("prevotes", updatedVoteCount)
                                else roundVotes.with("precommits", updatedVoteCount)
                                .with("votesAddressesWeights", updatedVotesAddressesWeights)
                                .with("emittedThresholds", roundVotes.emittedThresholds.setAdd(finalThreshold))
        val updatedBookkeeper = keeper.with("rounds", keeper.rounds.mapSafeSet(vote.round, updatedRoundVotes))
        {bookkeper: updatedBookkeeper, event: toEvent(vote.round, vote.typ, finalThreshold)}
    }

    pure def checkThreshold(keeper: Bookkeeper, round: Round, voteType: str, threshold: Threshold): bool = {
        if (keeper.rounds.has(round)) {
            val roundVotes = keeper.rounds.get(round)
            val voteCount = if (voteType == "Prevote") roundVotes.prevotes
                            else roundVotes.precommits
            val total = voteCount.total
            val sumWeight = voteCount.valuesWeights.keys().fold(0, (sum, v) => sum + voteCount.valuesWeights.get(v))
            if (threshold.name == "Value") isQuorum(voteCount.valuesWeights.getOrElse(threshold.value, 0), total)
            else if (threshold.name == "Nil") isQuorum(voteCount.valuesWeights.getOrElse("nil", 0), total)
            else if (threshold.name == "Any") isQuorum(sumWeight, total)
            else false
        } else false
    }
}