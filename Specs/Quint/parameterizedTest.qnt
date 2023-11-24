module parameterizedTest {

import statemachineAsync(
    validators = Set("v1", "v2", "v3", "v4"),
    validatorSet = Set("v1", "v2", "v3", "v4").mapBy(x => 1),
    Faulty = Set(),
    Values = Set("a", "b"),
    Rounds = Set(0, 1, 2, 3),
    Heights = Set(0) // , 1, 2, 3)
) as N4F0 from "./statemachineAsync"


// run everyoneTakesAStep (proposer, nonprop) = {
//     N4F0::valStep(nonprop._1)
//     .then(N4F0::valStep(nonprop._2))
//     .then(N4F0::valStep(nonprop._3))
//     .then(N4F0::valStep(proposer))
// }

run everyoneTakesAStep = {
    N4F0::valStep("v1")
    .then(N4F0::valStep("v2"))
    .then(N4F0::valStep("v3"))
    .then(N4F0::valStep("v4"))
}

run oneDeliversPrevote (validator, prop, r, proposer, nonprop) = {
    N4F0::deliverVote(validator, { height: 0, id: "nil", round: r, src: nonprop._1, step: "Prevote" })
    .then(N4F0::deliverVote(validator, { height: 0, id: "nil", round: r, src: nonprop._2, step: "Prevote" }))
    .then(N4F0::deliverVote(validator, { height: 0, id: "nil", round: r, src: nonprop._3, step: "Prevote" }))
    .then(N4F0::deliverVote(validator, { height: 0, id: prop, round: r, src: proposer, step: "Prevote" }))
}

run oneDeliversPrecommit (validator, prop, r, proposer, nonprop) = {
    N4F0::deliverVote(validator, { height: 0, id: "nil", round: r, src: nonprop._1, step: "Precommit" })
    .then(N4F0::deliverVote(validator, { height: 0, id: "nil", round: r, src: nonprop._2, step: "Precommit" }))
    .then(N4F0::deliverVote(validator, { height: 0, id: "nil", round: r, src: nonprop._3, step: "Precommit" }))
    .then(N4F0::deliverVote(validator, { height: 0, id: "nil", round: r, src: proposer, step: "Precommit" }))
}

run UnSuccessfulRound (prop: str, r: int) : bool = {
    val proposer = N4F0::Proposer(N4F0::validatorSet, 0, r)
    val nonprop =   if (proposer == "v1") ("v2", "v3", "v4")
                    else if (proposer == "v2") ("v1", "v3", "v4")
                    else if (proposer == "v3")  ("v1", "v2", "v4")
                    else ("v1", "v2", "v3")
    // everyone is in round r and proposer sent a proposal
    all {
        assert(N4F0::propBuffer.keys().forall(p => N4F0::propBuffer.get(p).contains(
            { height: 0, proposal: prop, round: r, src: proposer, validRound: -1 }) )),
        N4F0::unchangedAll
    }
    .then(N4F0::deliverProposal(proposer, { height: 0, proposal: prop, round: r, src: proposer, validRound: -1 }))
    .then(everyoneTakesAStep)
    .then(oneDeliversPrevote(proposer, prop, r, proposer, nonprop))
    .then(oneDeliversPrevote(nonprop._1, prop, r, proposer, nonprop))
    .then(oneDeliversPrevote(nonprop._2, prop, r, proposer, nonprop))
    .then(oneDeliversPrevote(nonprop._3, prop, r, proposer, nonprop))        
    .then(everyoneTakesAStep)
    .then(everyoneTakesAStep)
    .then(everyoneTakesAStep)
    .then(all {
        assert(N4F0::system.keys().forall(p => N4F0::system.get(p).timeout.contains(
            ("TimeoutPrevote", 0, r)) )),
        N4F0::unchangedAll
    })
    .then(everyoneTakesAStep)
    .then(oneDeliversPrecommit(proposer, prop, r, proposer, nonprop))
    .then(oneDeliversPrecommit(nonprop._1, prop, r, proposer, nonprop))
    .then(oneDeliversPrecommit(nonprop._2, prop, r, proposer, nonprop))
    .then(oneDeliversPrecommit(nonprop._3, prop, r, proposer, nonprop))    
    .then(everyoneTakesAStep)
    .then(everyoneTakesAStep)
    .then(everyoneTakesAStep) 
    .then(everyoneTakesAStep) 
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