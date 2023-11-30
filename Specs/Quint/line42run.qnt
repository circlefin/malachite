// -*- mode: Bluespec; -*-

module line42run {

import TendermintDSL.* from "./TendermintDSL"
export TendermintDSL.*

const testedVal : Address_t
val others = validators.exclude(Set(testedVal)) 
val othersList = others.fold(List(), (s, x) => s.append(x))

run runToLine42 = {
    val value = "blue"
    init
    .then(valStep(testedVal))
    .then(everyoneReceivesProposal(othersList, validatorList, validatorSet, 0, 0, value))
    .then(fromPrevoteToPrecommit(othersList, othersList, validatorList, validatorSet, 0, 0, value))
    // At this point we have set up the environment for "testedVal" to reach line 42 without
    // any other process taking any steps
    .then(all{
        assert(system.get(testedVal).timeout.contains(("TimeoutPropose", 0, 0))),
        valStep(testedVal)
    })
    .then(ProcessDeliverAllVotes("Prevote", testedVal, othersList, validatorSet, 0, 0, value))
    .then(othersList.length().reps(_ => valStep(testedVal)))
    .then(all{
        assert(system.get(testedVal).timeout.contains(("TimeoutPrevote", 0, 0))),
        valStep(testedVal) 
    })
    .then(all{
        assert(system.get(testedVal).es.cs.step == "Precommit"),
        deliverSomeProposal(testedVal)
    })
    .then(valStep(testedVal)) // here it executes line 36 with branch 42
    .then(all{
        val cstate = system.get(testedVal).es.cs
        assert(all{
            cstate.lockedRound == -1,
            cstate.lockedValue == "nil",
            cstate.validRound == 0,
            cstate.validValue == value
            }),
        unchangedAll
    }) 
}

}

