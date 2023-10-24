// -*- mode: Bluespec; -*-

module voteBookkeeper {

    import basicSpells.* from "./basicSpells"
    import extraSpells.* from "./extraSpells"

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
        value: Value
    }

    type VoteCount = {
        total: Weight,
        // includes nil
        valuesWeights: ValueWeights,
    }

    type RoundVotes = {
        height: Height,
        round: Round,
        prevotes: VoteCount,
        precommits: VoteCount,
    }

    type Threshold = {
        name: str,
        value: Value
    }


    type ExecutorEvent = {
        name: str,
        value: Value
    }

    type Bookkeeper = {
        height: Height,
        totalWeight: int,
        rounds: Round -> RoundVotes
    }

    // Internal functions

    pure def newVoteCount(total: Weight): VoteCount = {
        {total: total, valuesWeights: Map()}
    }

    pure def isQuorum(weight: int, total: int): bool = {
        3 * weight > 2 * total
    }

    pure def addVote(voteCount: VoteCount, vote: Vote, weight: Weight): {voteCount: VoteCount, threshold: Threshold} = {
        val value = vote.value
        val total = voteCount.total
        val newWeight = voteCount.valuesWeights.getOrElse(value, 0) + weight
        val updatedVoteCount = voteCount.with("valuesWeights", voteCount.valuesWeights.mapSafeSet(value, newWeight))
        val sumWeight = updatedVoteCount.valuesWeights.keys().fold(0, (sum, v) => sum + updatedVoteCount.valuesWeights.get(v))
        val threshold = if (value != "nil" and isQuorum(newWeight, total)) {name: "Value", value: value}
                        else if (value == "nil" and isQuorum(newWeight, total)) {name: "Nil", value: "null"}
                        else if (isQuorum(sumWeight, total)) {name: "Any", value: "null"}
                        else {name: "Unreached", value: "null"}
        {voteCount: updatedVoteCount, threshold: threshold}
    }

    pure def checkThresholdCount(voteCount: VoteCount, threshold: Threshold): bool = {
        val total = voteCount.total
        val sumWeight = voteCount.valuesWeights.keys().fold(0, (sum, v) => sum + voteCount.valuesWeights.get(v))
        if (threshold.name == "Value") isQuorum(voteCount.valuesWeights.getOrElse(threshold.value, 0), total)
        else if (threshold.name == "Nil") isQuorum(voteCount.valuesWeights.getOrElse("nil", 0), total)
        else if (threshold.name == "Any") isQuorum(sumWeight, total)
        else false
    }

    pure def toEvent(voteType: str, threshold: Threshold): ExecutorEvent = {
        if (threshold.name == "Unreached") {name: "None", value: "null"}
        else if (voteType == "Prevote" and threshold.name == "Value") {name: "PolkaValue", value: threshold.value}
        else if (voteType == "Prevote" and threshold.name == "Nil") {name: "PolkaNil", value: "null"}
        else if (voteType == "Prevote" and threshold.name == "Any") {name: "PolkaAny", value: "null"}
        else if (voteType == "Precommit" and threshold.name == "Value") {name: "PrecommitValue", value: threshold.value}
        else if (voteType == "Precommit" and threshold.name == "Any") {name: "PrecommitAny", value: "null"}
        else {name: "None", value: "null"}
    }

    // Executor interface

    pure def applyVote(keeper: Bookkeeper, vote: Vote, weight: int): {bookkeper: Bookkeeper, event: ExecutorEvent} = {
        val height = keeper.height
        val total = keeper.totalWeight
        val roundVotes = keeper.rounds.getOrElse(vote.round, {height: height, round: vote.round, prevotes: newVoteCount(total), precommits: newVoteCount(total)})
        val resultAddVote = if (vote.typ == "Prevote") roundVotes.prevotes.addVote(vote, weight)
                            else roundVotes.precommits.addVote(vote, weight)
        val updatedRoundVotes = if (vote.typ == "Prevote") roundVotes.with("prevotes", resultAddVote.voteCount)
                                else roundVotes.with("precommits", resultAddVote.voteCount)
        val updatedBookkeeper = keeper.with("rounds", keeper.rounds.mapSafeSet(vote.round, updatedRoundVotes))
        {bookkeper: updatedBookkeeper, event: toEvent(vote.typ, resultAddVote.threshold)}
    }

    pure def checkThreshold(keeper: Bookkeeper, round: Round, voteType: str, threshold: Threshold): bool = {
        if (keeper.rounds.has(round)) {
            val roundVotes = keeper.rounds.get(round)
            val voteCount = if (voteType == "Prevote") roundVotes.prevotes
                            else roundVotes.precommits
            checkThresholdCount(voteCount, threshold)
        } else false
    }

}