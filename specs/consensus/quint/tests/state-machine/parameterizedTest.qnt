// -*- mode: Bluespec; -*-

module parameterizedTest {

import types.* from "../../types"
import extraSpells.* from "../../spells/extra"
import statemachineAsync(
    validators = Set("v1", "v2", "v3", "v4"),
    validatorSet = Set("v1", "v2", "v3", "v4").mapBy(x => 1),
    Faulty = Set(),
    Values = Set("a", "b"),
    Rounds = Set(0, 1, 2, 3),
    Heights = Set(0) // , 1, 2, 3)
) as N4F0 from "../../statemachineAsync"

val validatorList = N4F0::validators.fold(List(), (s, x) => s.append(x))

run everyoneTakesAStep = {
    validatorList.length().reps(i => N4F0::valStep(validatorList[i]))
}

run everyoneTimesOut = {
    validatorList.length().reps(i => N4F0::valTimeout(validatorList[i]))
}

// here there is a different prevote from the proposer
run oneDeliversPrevote(validator, prop, r, proposer, nonprop) =
    nonprop.length().reps(i => 
        N4F0::deliverVote(validator, mkVote(Prevote, nonprop[i], 0, r, Nil)))
    .then(N4F0::deliverVote(validator, mkVote(Prevote, proposer, 0, r, Val(prop))))

run everyoneDeliversPrevote(prop, r, proposer, nonprop) =
    validatorList.length().reps(i => 
        oneDeliversPrevote(validatorList[i], prop, r, proposer, nonprop))    

// all the precommits are for "nil"
run oneDeliversPrecommit(validator, prop, r, proposer, nonprop) =
    validatorList.length().reps(i => 
        N4F0::deliverVote(validator, mkVote(Precommit, validatorList[i], 0, r, Nil)))

run everyoneDeliversPrecommit(prop, r, proposer, nonprop) =
    validatorList.length().reps(i => 
        oneDeliversPrecommit(validatorList[i], prop, r, proposer, nonprop))

run UnSuccessfulRound(prop: NonNilValue, r: Round): bool =
    val proposer = N4F0::proposer(N4F0::validatorSet, 0, r)
    val nonprop = validatorList.select(x => x != proposer)
    // everyone is in round r and proposer sent a proposal
    all {
        assert(N4F0::propBuffer.forallValues(ps => ps.contains(mkProposal(proposer, 0, r, prop, -1)))),
        N4F0::unchangedAll
    }
    .then(N4F0::deliverProposal(proposer, mkProposal(proposer, 0, r, prop, -1)))
    .then(N4F0::valStep(proposer))
    .then(nonprop.length().reps(i => N4F0::valTimeout(nonprop[i])))
    .then(all{
        assert(N4F0::system.keys().forall(p => N4F0::system.get(p).es.pendingStepChange == PrevoteStep)),
        everyoneTakesAStep
    })
    .then(everyoneDeliversPrevote(prop, r, proposer, nonprop))  
    .then(3.reps(i => everyoneTakesAStep))
    .then(all {
        assert(N4F0::system.forallValues(state => state.timeouts.contains((PrevoteTimeout, 0, r)))),
        N4F0::unchangedAll
    })
    .then(everyoneTimesOut)
    .then(all{
        assert(N4F0::system.keys().forall(p => N4F0::system.get(p).es.pendingStepChange == PrecommitStep)),
        everyoneTakesAStep
    })
    .then(everyoneDeliversPrecommit(prop, r, proposer, nonprop))
    .then(4.reps(i => everyoneTakesAStep))
    .then(all {
        assert(N4F0::system.forallValues(state => state.timeouts.contains((PrecommitTimeout, 0, r)))),
        N4F0::unchangedAll
    })

run UnSuccessfulRoundWithSetup (prop: NonNilValue, r: Round): bool =
    N4F0::setNextValueToPropose(N4F0::proposer(N4F0::validatorSet, 0, r), prop)
    .then(everyoneTakesAStep)
    .then(all{ // after new round an empty step to clean step "propose"
        assert(N4F0::system.keys().forall(p => N4F0::system.get(p).es.pendingStepChange == ProposeStep)),
        everyoneTakesAStep
    })
    .then(all {
        assert(N4F0::system.forallValues(state => state.es.cs.round == r)),
        UnSuccessfulRound (prop, r)
    })

run UnSuccessfulRoundWithSetupTO (prop: NonNilValue, r: Round): bool =
    N4F0::setNextValueToPropose(N4F0::proposer(N4F0::validatorSet, 0, r), prop)
    .then(everyoneTimesOut)
    .then(all{ // after new round an empty step to clean step "propose"
        assert(N4F0::system.keys().forall(p => N4F0::system.get(p).es.pendingStepChange == ProposeStep)),
        everyoneTakesAStep
    })
    .then(all {
        assert(N4F0::system.forallValues(state => state.es.cs.round == r)),
        UnSuccessfulRound (prop, r)
    })

run oneOneRoundTest = {
    N4F0::init    
    .then(UnSuccessfulRoundWithSetup("blue", 0))
}

run multiRoundTest = {
    val proposals = ["blue", "red", "green", "yellow"]
    val repetitions = proposals.length() - 1
    N4F0::init    
    .then(UnSuccessfulRoundWithSetup(proposals[0], 0))
    .then(repetitions.reps(i => UnSuccessfulRoundWithSetupTO(proposals[i + 1], i + 1)))
}



}
