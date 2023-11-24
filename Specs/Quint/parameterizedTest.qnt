module parameterizedTest {

import statemachineAsync(
    validators = Set("v1", "v2", "v3", "v4"),
    validatorSet = Set("v1", "v2", "v3", "v4").mapBy(x => 1),
    Faulty = Set(),
    Values = Set("a", "b"),
    Rounds = Set(0, 1, 2, 3),
    Heights = Set(0) // , 1, 2, 3)
) as N4F0 from "./statemachineAsync"

val validatorList = N4F0::validators.fold(List(), (s, x) => s.append(x))

run everyoneTakesAStep = {
    validatorList.length().reps(i => N4F0::valStep(validatorList[i]))
}

// here there is a different prevote from the proposer
run oneDeliversPrevote (validator, prop, r, proposer, nonprop) = {
    nonprop.length().reps( i => 
        N4F0::deliverVote(validator, { height: 0, id: "nil", round: r, src: nonprop[i], step: "Prevote" }))
    .then(N4F0::deliverVote(validator, { height: 0, id: prop, round: r, src: proposer, step: "Prevote" }))
}

run everyoneDeliversPrevote (prop, r, proposer, nonprop) = {
    validatorList.length().reps(i => oneDeliversPrevote (validatorList[i], prop, r, proposer, nonprop))    
}

// all the precommits are for "nil"
run oneDeliversPrecommit (validator, prop, r, proposer, nonprop) = {
    validatorList.length().reps(i => 
        N4F0::deliverVote(validator, { height: 0, id: "nil", round: r, src: validatorList[i], step: "Precommit" }))
}

run everyoneDeliversPrecommit  (prop, r, proposer, nonprop) = {
    validatorList.length().reps(i => oneDeliversPrecommit (validatorList[i], prop, r, proposer, nonprop))    
}

run UnSuccessfulRound (prop: str, r: int) : bool = {
    val proposer = N4F0::Proposer(N4F0::validatorSet, 0, r)
    val nonprop = validatorList.select(x => x != proposer)
    // everyone is in round r and proposer sent a proposal
    all {
        assert(N4F0::propBuffer.keys().forall(p => N4F0::propBuffer.get(p).contains(
            { height: 0, proposal: prop, round: r, src: proposer, validRound: -1 }) )),
        N4F0::unchangedAll
    }
    .then(N4F0::deliverProposal(proposer, { height: 0, proposal: prop, round: r, src: proposer, validRound: -1 }))
    .then(everyoneTakesAStep)
    .then(everyoneDeliversPrevote(prop, r, proposer, nonprop))  
    .then(3.reps(i => everyoneTakesAStep))
    .then(all {
        assert(N4F0::system.keys().forall(p => N4F0::system.get(p).timeout.contains(
            ("TimeoutPrevote", 0, r)) )),
        N4F0::unchangedAll
    })
    .then(everyoneTakesAStep)
    .then(everyoneDeliversPrecommit(prop, r, proposer, nonprop))
    .then(4.reps(i => everyoneTakesAStep))
    .then(all {
        assert(N4F0::system.keys().forall(p => N4F0::system.get(p).timeout.contains(
            ("TimeoutPrecommit", 0, r)) )),
        N4F0::unchangedAll
    })    
} 

run UnSuccessfulRoundWithSetup (prop: str, r: int) : bool = {
    N4F0::setNextValueToPropose(N4F0::Proposer(N4F0::validatorSet, 0, r), prop)
    .then(everyoneTakesAStep)
    .then(all {
        assert(N4F0::system.keys().forall(p => N4F0::system.get(p).es.cs.round == r)),
        UnSuccessfulRound (prop, r)
    })  
}

run multiRoundTest = {
    val proposals = ["blue", "red", "green", "yellow"]
    val repetitions = proposals.length()
    N4F0::init    
    .then(repetitions.reps(i => UnSuccessfulRoundWithSetup(proposals[i], i)))
}


}