// -*- mode: Bluespec; -*-

// Positive mirror of tests/disagreement/disagreementRun.qnt.
//
// disagreementRun drives n=4, f=2 (BEYOND n > 3f) and asserts `not(AgreementInv)`
// -- i.e. it exhibits a fork. This run drives n=4, f=1 (WITHIN n > 3f) on the
// happy path where the correct nodes (a 2f+1 quorum) decide the same value, and
// asserts that `AgreementInv` HOLDS. It is the first positive agreement check in
// the suite; CI runs it via `quint test` (see tests/agreement/agreementTest.qnt).
//
// This is a single-trace witness, not a proof. The proof of AgreementInv for all
// executions is the inductive-invariant work in agreementInductive.qnt.

module agreementRun {

import TendermintDSL.* from "../../TendermintDSL"
export TendermintDSL.*

// Everyone agrees: a single value is proposed, prevoted, and precommitted by the
// whole correct quorum (plus the faulty node echoing it), so all correct nodes
// decide the same value at height 0.
run AgreementRun = {
    val thisProposer = proposer(validatorSet, 0, 0)
    nondet value = oneOf(Values)
    val allVoters = correctList.concat(faultyList)
    init
        .then(2.reps(_ => ListTakeAStep(correctList)))
        .then(ListDeliverProposal(correctList, mkProposal(thisProposer, 0, 0, value, -1)))
        .then(ListTakeAStep(correctList))
        .then(ListTakeAStep(correctList)) // consume step change
        // a polka: everyone prevotes the same value
        .then(ListDeliverAllVotes(Prevote, allVoters, correctList, validatorSet, 0, 0, Val(value)))
        .then(allVoters.length().reps(_ => ListTakeAStep(correctList)))
        .then(ListTakeAStep(correctList)) // timeout prevote started -> consume pending
        .then(ListTakeAStep(correctList)) // consume step change
        // a commit quorum: everyone precommits the same value
        .then(ListDeliverAllVotes(Precommit, allVoters, correctList, validatorSet, 0, 0, Val(value)))
        .then(allVoters.length().reps(_ => ListTakeAStep(correctList)))
        .then(all {
            // Non-vacuity: the correct nodes actually reached a decision...
            assert(Correct.forall(v => system.get(v).es.chain.length() > 0)),
            // ...and the property under test holds: no two correct validators disagree.
            assert(AgreementInv),
            unchangedAll
        })
}

}
