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

    // A value is a string
    type Value = str

    // Weigth is an integer
    type Weight = int

    // The vote type
    type Vote = {
        typ: str,
        round: Round,
        value: Value,
        address: Address
    }

    type VoteCount = {
        totalWeight: Weight,
        // includes nil
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

    // ****************************************************************************
    // Functional Layer
    // ****************************************************************************

    // Internal functions

    // creates a new voteCount
    pure def newVoteCount(total: Weight): VoteCount = {
        {totalWeight: total, valuesWeights: Map(), votesAddresses: Set()}
    }

    // Returns true if weight > 2/3 * total (quorum: at least f+1 correct)
    pure def isQuorum(weight: int, total: int): bool = {
        3 * weight > 2 * total
    }

    // Returns true if weight > 1/3 * total (small quorum: at least one correct)
    pure def isSkip(weight: int, total: int): bool = {
        3 * weight > total
    }
    // Adds a vote of weight weigth to a voteCount if there is not vote registered for the voter.
    pure def addVote(voteCount: VoteCount, vote: Vote, weight: Weight): VoteCount = {
        if (vote.address.in(voteCount.votesAddresses)) voteCount
        else val newWeight = voteCount.valuesWeights.getOrElse(vote.value, 0) + weight             
             voteCount.with("valuesWeights", voteCount.valuesWeights.mapSafeSet(vote.value, newWeight))
                      .with("votesAddresses", voteCount.votesAddresses.setAdd(vote.address))
    }

    // Given a voteCount and a value, the function returns:
    // - A threshold Value if there is a quorum for the given value;
    // - A threshold Nil if there is a quorum for the nil and no quorum for the value;
    // - A threshold Any if there is no quorum for the value or nil and there is a quroum for any (including nil);
    // - A threshold Unreached otherwise indicating that no quorum has been yet reached.
    pure def computeThreshold(voteCount: VoteCount, value: Value): Threshold = {
        val weight = voteCount.valuesWeights.getOrElse(value, 0)
        val totalWeight = voteCount.totalWeight
        val sumWeight = voteCount.valuesWeights.keys().fold(0, (sum, v) => sum + voteCount.valuesWeights.get(v))
        if (value != "nil" and isQuorum(weight, totalWeight)) {name: "Value", value: value}
        else if (value == "nil" and isQuorum(weight, totalWeight)) {name: "Nil", value: "null"}
        else if (isQuorum(sumWeight, totalWeight)) {name: "Any", value: "null"}
        else {name: "Unreached", value: "null"}
    }

    // Given a round, voteType and threshold it resturns the corresponding ExecutorEvent
    pure def toEvent(round: Round, voteType: str, threshold: Threshold): ExecutorEvent = {
        if (threshold.name == "Unreached") {round: round, name: "None", value: "null"}
        else if (voteType == "Prevote" and threshold.name == "Value") {round: round, name: "PolkaValue", value: threshold.value}
        else if (voteType == "Prevote" and threshold.name == "Nil") {round: round, name: "PolkaNil", value: "null"}
        else if (voteType == "Prevote" and threshold.name == "Any") {round: round, name: "PolkaAny", value: "null"}
        else if (voteType == "Precommit" and threshold.name == "Value") {round: round, name: "PrecommitValue", value: threshold.value}
        else if (voteType == "Precommit" and threshold.name == "Any") {round: round, name: "PrecommitAny", value: "null"}
        else if (threshold.name == "Skip") {round: round, name: "Skip", value: "null"}
        else {round: round, name: "None", value: "null"}
    }



    // Executor interface

    // Called by the executor when it receives a vote. The functiojn takes the following steps:
    // - It first adds the vote and then computes a threshold.
    // - If there exist a threshold and has not emitted before, the function returns the corresponsing ExecutorEvent.
    // - Othewise, the function returns a no-threshold event.
    // - Note that if there is no threshold after adding the vote, the function checks if there is a skip threshold.
    // TO DISCUSS:
    // - There might be a problem if we generalize from single-shot to multi-shot: the keeper only keeps the totalWeight
    //   of the current height; I wonder if we need to keep the totalWeight for every Height that we may receive a vote for.
    pure def applyVote(keeper: Bookkeeper, vote: Vote, weight: int): {bookkeeper: Bookkeeper, event: ExecutorEvent} = {
        val height = keeper.height
        val total = keeper.totalWeight
        val roundVotes = keeper.rounds.getOrElse(vote.round, {height: height,
                                                              round: vote.round,
                                                              prevotes: newVoteCount(total),
                                                              precommits: newVoteCount(total),
                                                              emittedEvents: Set(),
                                                              votesAddressesWeights: Map()})
        val updatedVoteCount = if (vote.typ == "Prevote") roundVotes.prevotes.addVote(vote, weight)
                               else roundVotes.precommits.addVote(vote, weight)
        val threshold = computeThreshold(updatedVoteCount, vote.value)
        val event = toEvent(vote.round, vote.typ, threshold)
        val updatedVotesAddressesWeights = if (roundVotes.votesAddressesWeights.has(vote.address)) roundVotes.votesAddressesWeights
                                           else roundVotes.votesAddressesWeights.mapSafeSet(vote.address, weight)
        val sumSkip = updatedVotesAddressesWeights.keys().fold(0, (sum, v) => sum + updatedVotesAddressesWeights.get(v))
        val finalEvent = if (not(event.in(roundVotes.emittedEvents))) event
                         else if (roundVotes.emittedEvents == Set() and isSkip(sumSkip, total)) {round: vote.round, name: "Skip", value: "null"}
                         else {round: vote.round, name: "None", value: "null"}
        val updatedRoundVotes = if (vote.typ == "Prevote") roundVotes.with("prevotes", updatedVoteCount)                                   
                                else roundVotes.with("precommits", updatedVoteCount)
        val updatedEmmittedEvents = if (finalEvent.name != "None") roundVotes.emittedEvents.setAdd(finalEvent)
                                    else roundVotes.emittedEvents
        val updatedBookkeeper = keeper.with("rounds", keeper.rounds.mapSafeSet(vote.round, updatedRoundVotes.with("votesAddressesWeights", updatedVotesAddressesWeights)
                                                                                                            .with("emittedEvents", updatedEmmittedEvents)))
        {bookkeeper: updatedBookkeeper, event: finalEvent}
    }
    
    // Called by the executor to check if there is a specific threshold for a given round and voteType.
    // TO DISCUSS:
    // - The function does not consider Skip threshold. This because if the executor receives a Skip event
    //   and do not act on it, this means that it will never do it in the future. We should discuss that this
    //   is the case.
    pure def checkThreshold(keeper: Bookkeeper, round: Round, voteType: str, threshold: Threshold): bool = {
        if (keeper.rounds.has(round)) {
            val roundVotes = keeper.rounds.get(round)
            val voteCount = if (voteType == "Prevote") roundVotes.prevotes
                            else roundVotes.precommits
            val total = voteCount.totalWeight
            val sumWeight = voteCount.valuesWeights.keys().fold(0, (sum, v) => sum + voteCount.valuesWeights.get(v))
            if (threshold.name == "Value") isQuorum(voteCount.valuesWeights.getOrElse(threshold.value, 0), total)
            else if (threshold.name == "Nil") isQuorum(voteCount.valuesWeights.getOrElse("nil", 0), total)
            else if (threshold.name == "Any") isQuorum(sumWeight, total)
            else false
        } else false
    }

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

    run addVoteTest = {
        val voteCount = {totalWeight: 100, valuesWeights: Map("val1" -> 30, "val2" -> 20), votesAddresses: Set("alice", "bob")}
        val vote = {typ: "precommit", round: 10, value: "val3", address: "john"}
        all {
            // new voter, new value
            assert(addVote(voteCount, vote, 10) == {totalWeight: 100, valuesWeights: Map("val1" -> 30, "val2" -> 20, "val3" -> 10), votesAddresses: Set("alice", "bob", "john")}),
            // new voter, existing value
            assert(addVote(voteCount, vote.with("value", "val2"), 10) == {totalWeight: 100, valuesWeights: Map("val1" -> 30, "val2" -> 30), votesAddresses: Set("alice", "bob", "john")}),
            // existing voter
            assert(addVote(voteCount, vote.with("address", "alice"), 10) == voteCount),
        }
    }

    run computeThresholdTest = {  
        val voteCount = {totalWeight: 100, valuesWeights: Map(), votesAddresses: Set("alice", "bob")}
        val mapValueReached = Map("val1" -> 67, "val2" -> 20)
        val mapNilReached = Map("nil" -> 70, "val2" -> 20)
        val mapNoneReached = Map("nil" -> 20, "val2" -> 20)

        all {
            assert(computeThreshold(voteCount, "val3") == {name: "Unreached", value: "null"}),
            assert(computeThreshold(voteCount.with("valuesWeights", mapValueReached), "val1") == {name: "Value", value: "val1"}),
            assert(computeThreshold(voteCount.with("valuesWeights", mapValueReached), "val2") == {name: "Any", value: "null"}),
            assert(computeThreshold(voteCount.with("valuesWeights", mapNilReached), "nil") == {name: "Nil", value: "null"}),
            assert(computeThreshold(voteCount.with("valuesWeights", mapNilReached), "val2") == {name: "Any", value: "null"}),
            assert(computeThreshold(voteCount.with("valuesWeights", mapNoneReached), "val1") == {name: "Unreached", value: "null"}),
            assert(computeThreshold(voteCount.with("valuesWeights", mapNoneReached), "nil") == {name: "Unreached", value: "null"}),
        }
    }

    run toEventTest = {  
        val thresholdUnreached = {name: "Unreached", value: "null"}
        val thresholdAny = {name: "Any", value: "null"}
        val thresholdNil = {name: "Nil", value: "null"}
        val thresholdValue = {name: "Value", value: "val1"}
        val thresholdSkip = {name: "Skip", value: "null"}
        val round = 10
        all {
            assert(toEvent(round, "Prevote", thresholdUnreached) == {round: round, name: "None", value: "null"}),
            assert(toEvent(round, "Precommit", thresholdUnreached) == {round: round, name: "None", value: "null"}),
            assert(toEvent(round, "Prevote", thresholdAny) == {round: round, name: "PolkaAny", value: "null"}),
            assert(toEvent(round, "Prevote", thresholdNil) == {round: round, name: "PolkaNil", value: "null"}),
            assert(toEvent(round, "Prevote", thresholdValue) == {round: round, name: "PolkaValue", value: "val1"}),
            assert(toEvent(round, "Precommit", thresholdAny) == {round: round, name: "PrecommitAny", value: "null"}),
            assert(toEvent(round, "Precommit", thresholdNil) == {round: round, name: "None", value: "null"}),
            assert(toEvent(round, "Precommit", thresholdValue) == {round: round, name: "PrecommitValue", value: "val1"}),
            assert(toEvent(round, "Prevote", thresholdSkip) == {round: round, name: "Skip", value: "null"}),
            assert(toEvent(round, "Precommit", thresholdSkip) == {round: round, name: "Skip", value: "null"}),
            assert(toEvent(round, "Precommit", {name: "Error", value: "null"}) == {round: round, name: "None", value: "null"}),
            assert(toEvent(round, "Error", thresholdAny) == {round: round, name: "None", value: "null"}),
        }
    }

    // ****************************************************************************
    // State machine state
    // **************************************************************************** 

    // Bookkeeper state
    var bookkeeper: Bookkeeper
    // Last emitted event
    var lastEmitted: ExecutorEvent

    // ****************************************************************************
    // Execution
    // ****************************************************************************

    action allUnchanged: bool = all {
      bookkeeper' = bookkeeper,
      lastEmitted' = lastEmitted
    }

    action init(totalWeight: Weight): bool = all {
        bookkeeper' = {height: 10, totalWeight: totalWeight, rounds: Map()},
        lastEmitted' = {round: -1, name: "", value: "null"}
    }

    action applyVoteAction(vote: Vote, weight: Weight): bool = all {
        val result = applyVote(bookkeeper, vote, weight)
        all {
            bookkeeper' = result.bookkeeper,
            lastEmitted' = result.event
        }   
    }

    // ****************************************************************************
    // Test traces
    // ****************************************************************************

    // Consensus full execution with all honest validators (including the leader) and a synchronous network:
    // all messages are received in order. We assume three validators in the validator set wtih 60%, 30% and 10%
    // each of the total voting power
    run synchronousConsensusTest = {
        init(100)
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "proposal", address: "alice"}, 60))
        .then(all{ assert(lastEmitted == {round: 1, name: "None", value: "null"}), allUnchanged })
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "proposal", address: "john"}, 10))
        .then(all{ assert(lastEmitted == {round: 1, name: "PolkaValue", value: "proposal"}), allUnchanged })
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "proposal", address: "bob"}, 30))
        .then(all{ assert(lastEmitted == {round: 1, name: "None", value: "null"}), allUnchanged })
        .then(applyVoteAction({typ: "Precommit", round: 1, value: "proposal", address: "bob"}, 30))
        .then(all{ assert(lastEmitted == {round: 1, name: "None", value: "null"}), allUnchanged })
        .then(applyVoteAction({typ: "Precommit", round: 1, value: "proposal", address: "john"}, 10))
        .then(all{ assert(lastEmitted == {round: 1, name: "None", value: "null"}), allUnchanged })
        .then(applyVoteAction({typ: "Precommit", round: 1, value: "proposal", address: "alive"}, 60))
        .then(all{ assert(lastEmitted == {round: 1, name: "PrecommitValue", value: "proposal"}), allUnchanged })
    }

    // Reaching PolkaAny 
    run polkaAnyTest = {
        init(100)
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "val1", address: "alice"}, 60))
        .then(all{ assert(lastEmitted == {round: 1, name: "None", value: "null"}), allUnchanged })
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "nil", address: "john"}, 10))
        .then(all{ assert(lastEmitted == {round: 1, name: "PolkaAny", value: "null"}), allUnchanged })
    }

    // Reaching PolkaNil
    run polkaNilTest = {
        init(100)
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "nil", address: "alice"}, 60))
        .then(all{ assert(lastEmitted == {round: 1, name: "None", value: "null"}), allUnchanged })
        .then(applyVoteAction({typ: "Prevote", round: 1, value: "nil", address: "john"}, 10))
        .then(all{ assert(lastEmitted == {round: 1, name: "PolkaNil", value: "null"}), allUnchanged })
    }
}