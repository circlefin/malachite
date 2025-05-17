// -*- mode: Bluespec; -*-

module line42run {

import TendermintDSL.* from "../../TendermintDSL"
export TendermintDSL.*

const testedVal : Address
val others = validators.exclude(Set(testedVal)) 
val othersList = others.fold(List(), (s, x) => s.append(x))

run runToLine42 =
    nondet value = oneOf(Values)
    all{
        init,
        // in this test we need testedVal to be different from proposer
        assert(testedVal != proposer(validatorSet, 0, 0)) 
    }
    .then(valStep(testedVal))
    .then(everyoneReceivesProposal(othersList, validatorList, validatorSet, 0, 0, value))
    .then(fromPrevoteToPrecommit(othersList, othersList, validatorList, validatorSet, 0, 0, Val(value)))
    // At this point we have set up the environment for "testedVal" to reach line 42 without
    // any other process taking any steps
    .then(all{ 
        assert(system.get(testedVal).es.pendingStepChange == ProposeStep),
        valStep(testedVal)
    })     
    .then(all{
        assert(system.get(testedVal).timeouts.contains((ProposeTimeout, 0, 0))),
        valTimeout(testedVal)
    })
    .then(all{ 
        assert(system.get(testedVal).es.pendingStepChange == PrevoteStep),
        valStep(testedVal)
    })   
    .then(ProcessDeliverAllVotes(Prevote, testedVal, othersList, validatorSet, 0, 0, Val(value)))
    .then(othersList.length().reps(_ => valStep(testedVal)))
    .then(all{
        assert(system.get(testedVal).timeouts.contains((PrevoteTimeout, 0, 0))),
        valTimeout(testedVal) 
    })
    .then(all{ 
        assert(system.get(testedVal).es.pendingStepChange == PrecommitStep),
        valStep(testedVal)
    })      
    .then(all{
        assert(system.get(testedVal).es.cs.step == PrecommitStep),
        deliverSomeProposal(testedVal)
    })
    .then(valStep(testedVal)) // here it executes line 36 with branch 42
    .then(all{
        val cstate = system.get(testedVal).es.cs
        assert(all{
            cstate.lockedRound == -1,
            cstate.lockedValue == Nil,
            cstate.validRound == 0,
            cstate.validValue == Val(value),
        }),
        unchangedAll
    })

}
