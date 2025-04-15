// -*- mode: Bluespec; -*-

module line32run {

import TendermintDSL.* from "../../TendermintDSL"
export TendermintDSL.*

const testedVal : Address
const slowByz : Address


run runToLine32invalid = 
// the driver does not store invalid proposals. So we need to receive prevotes first and then the proposal
// first we send the correct guy to round 1
    val nextProposer = proposer(validatorSet, 0, 1)
    val propVal = "invalid"
    init
    .then(3.reps(_ => valStep(testedVal))) // start, pending step, 
    // consume timeout
    .then(valTimeout(testedVal))
    .then(deliverVote(testedVal, mkVote(Prevote, testedVal, 0, 0, Nil)))
    .then(ProcessDeliverAllVotes(Prevote, testedVal, faultyList.select(x => x != slowByz), validatorSet, 0, 0, Val(propVal)))
    .then(faultyList.select(x => x != slowByz).length().reps(_ => valStep(testedVal)))  
    .then(ProcessDeliverAllVotes(Precommit, testedVal, faultyList, validatorSet, 0, 0, Nil))
    .then(valStep(testedVal))
    .then(faultyList.length().reps(_ => valStep(testedVal))) 
    .then(
        all{
            assert(system.get(testedVal).timeouts.contains((PrecommitTimeout, 0, 0))),
            valTimeout(testedVal)
        }
    )
    .then(3.reps(_ => valStep(testedVal)))
    .then(deliverVote(testedVal, mkVote(Prevote, slowByz, 0, 0, Val(propVal))))
    .then(valStep(testedVal))
    .then(deliverProposal(testedVal, mkProposal(nextProposer, 0, 1, propVal, 0))) 
    .then(valStep(testedVal))
    .then(
        all{
            assert(system.get(testedVal).es.cs.step == PrevoteStep),
            assert(voteBuffer.get(testedVal).contains({ height: 0, round: 1, srcAddress: testedVal, valueId: Nil, voteType: Prevote })),            
            unchangedAll
        }

    )


run runToLine32locked = 
    // we need proposer of round 2 is the only one to lock in round 0 (thirdProposer)
    // other correct processes lock a different value in round 1
    // in round 2 the proposer proposes its locked value, but the other correct processes prevote nil now
    val nextProposer = proposer(validatorSet, 0, 1)
    val thirdProposer = proposer(validatorSet, 0, 2)
    nondet value = oneOf(Values)
    nondet nextValue = oneOf(Values.exclude(Set(value)))
    init
    .then(all {
        everyoneReceivesProposal(correctList, validatorList, validatorSet, 0, 0, value)
    })
    // thirdProposer receives all prevotes    
    .then(ProcessDeliverAllVotes(Prevote, thirdProposer, correctList, validatorSet, 0, 0, Val(value)))
    .then(correctList.length().reps(_ => valStep(thirdProposer)))
    .then(2.reps(_ => valStep(thirdProposer)))
    // now third proposer as locked and consumed step change
    // we now send a message from the faulty process (slowByz) to the other correct ones to send then through timeouts
    .then(ListDeliverVote(correctList.select(x => x != thirdProposer), mkVote(Prevote, slowByz, 0, 0, Nil)))
    .then(ListTakeAStep(correctList.select(x => x != thirdProposer)))
    .then(ListDeliverAllVotes(Prevote, correctList.select(x => x != thirdProposer), correctList.select(x => x != thirdProposer), validatorSet, 0, 0, Val(value)))
    .then(correctList.length().reps(_ => ListTakeAStep(correctList.select(x => x != thirdProposer)))) // process 2f correct and 1 faulty prevotes processed
    .then(ListTimesOut(correctList.select(x => x != thirdProposer))) // process timeout
    .then(ListTakeAStep(correctList.select(x => x != thirdProposer))) // consume step change
    .then(ListDeliverVote(correctList, mkVote(Precommit, thirdProposer, 0, 0, Val(value))))
    .then(ListDeliverAllVotes(Precommit, correctList.select(x => x != thirdProposer), correctList, validatorSet, 0, 0, Nil))
    .then(correctList.length().reps(_ => ListTakeAStep(correctList)))
    // here all correct have precommit timeout started
    // thirdproposer has locked value
    // we go to the next round
    
    .then(all {
        everyoneReceivesProposalTO(correctList, validatorList, validatorSet, 0, 1, nextValue)
    })
    .then(ListDeliverVote(correctList.select(x => x != thirdProposer), mkVote(Prevote, slowByz, 0, 1, Val(nextValue))))
    .then(ListDeliverAllVotes(Prevote, correctList.select(x => x != thirdProposer), correctList.select(x => x != thirdProposer), validatorSet, 0, 1, Val(nextValue)))
    .then(correctList.length().reps(_ => ListTakeAStep(correctList.select(x => x != thirdProposer))))
    .then(2.reps(_ => ListTakeAStep(correctList.select(x => x != thirdProposer))))
    .then(ListDeliverVote(correctList, mkVote(Precommit, slowByz, 0, 1, Nil)))
    .then(ListDeliverAllVotes(Precommit, correctList.select(x => x != thirdProposer), correctList, validatorSet, 0, 1, Val(nextValue)))
    .then(correctList.length().reps(_ => ListTakeAStep(correctList)))    
    // here all correct have precommit timeout started
    // thirdproposer is locked on value "value" from round 0
    // other correct processes have locked "nextValue" in round 1
    // now we deliver the prevote from thirdProposer so that others have Polka for round 0
    .then(ListDeliverVote(correctList.select(x => x != thirdProposer), mkVote(Prevote, thirdProposer, 0, 0, Val(value))))
    .then(ListTakeAStep(correctList.select(x => x != thirdProposer)))
    // we go to the next round
   .then(all {
        everyoneReceivesProposalVR(correctList, validatorList, validatorSet, 0, 2, value, 0)
    })
    .then(all { // the others prevoted Nil because they had a more recent lock
        assert(SetFromList(correctList.select(x => x != thirdProposer)).forall(x => 
                system.get(x).es.cs.step == PrevoteStep and
                voteBuffer.get(x).contains(mkVote(Prevote, x, 0, 2, Nil))
                )),
        unchangedAll
    })

}
