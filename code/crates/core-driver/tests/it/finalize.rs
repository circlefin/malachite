use malachitebft_core_state_machine::state::Step;
use malachitebft_core_types::{NilOrVal, Round, Validity};
use malachitebft_test::utils::validators::make_validators;
use malachitebft_test::{Address, Height, PrivateKey, TestContext, Validator, ValidatorSet, Value};

use informalsystems_malachitebft_core_driver::{Driver, Input, Output};

use crate::basic::{new_signed_precommit, new_signed_prevote, new_signed_proposal};

fn advance_driver_to_commit_step(
    value: Value,
    validators: [(Validator, PrivateKey); 3],
) -> (Driver<TestContext>, Address, Address, Address) {
    let [(v1, sk1), (v2, _sk2), (v3, _sk3)] = validators;
    let (_my_sk, my_addr) = (sk1, v1.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);
    let mut driver = Driver::new(ctx, height, vs.clone(), my_addr, Default::default());

    let proposal = new_signed_proposal(
        Height::new(1),
        Round::new(0),
        value.clone(),
        Round::Nil,
        my_addr,
    );

    // Reach Commit step
    let _ = driver.process(Input::NewRound(Height::new(1), Round::new(0), my_addr));
    let _ = driver.process(Input::Proposal(proposal.clone(), Validity::Valid));
    let _ = driver.process(Input::Vote(new_signed_prevote(
        Height::new(1),
        Round::new(0),
        NilOrVal::Val(value.id()),
        v2.address,
    )));
    let _ = driver.process(Input::Vote(new_signed_prevote(
        Height::new(1),
        Round::new(0),
        NilOrVal::Val(value.id()),
        v3.address,
    )));
    let _ = driver.process(Input::Vote(new_signed_precommit(
        Height::new(1),
        Round::new(0),
        NilOrVal::Val(value.id()),
        v2.address,
    )));
    let outputs = driver
        .process(Input::Vote(new_signed_precommit(
            Height::new(1),
            Round::new(0),
            NilOrVal::Val(value.id()),
            v3.address,
        )))
        .unwrap();

    // Verify we're in Commit step with a decision
    assert_eq!(driver.round_state().step, Step::Commit);
    assert!(outputs.iter().any(|o| matches!(o, Output::Decide(..))));

    (driver, my_addr, v2.address, v3.address)
}

#[test]
fn transition_to_finalize_from_commit() {
    let value = Value::new(9999);
    let validators = make_validators([1, 2, 3]);
    let (mut driver, _my_addr, _v2_addr, _v3_addr) =
        advance_driver_to_commit_step(value, validators);

    // Transition to Finalize
    let outputs = driver.process(Input::TransitionToFinalize).unwrap();

    // Verify transition successful
    assert!(outputs.is_empty(), "Transition should produce no outputs");
    assert_eq!(
        driver.round_state().step,
        Step::Finalize,
        "Should be in Finalize step"
    );
}

#[test]
fn finalize_step_rejects_all_inputs() {
    let value = Value::new(9999);
    let validators = make_validators([1, 2, 3]);
    let (mut driver, my_addr, v2_addr, _v3_addr) =
        advance_driver_to_commit_step(value.clone(), validators);

    // Transition to Finalize
    let _ = driver.process(Input::TransitionToFinalize);
    assert_eq!(driver.round_state().step, Step::Finalize);

    // Try various inputs - all should be rejected (return empty outputs)
    let vote = new_signed_precommit(Height::new(1), Round::new(0), NilOrVal::Nil, v2_addr);
    let outputs = driver.process(Input::Vote(vote)).unwrap();
    assert!(outputs.is_empty(), "Finalize step should reject Vote input");

    let proposal = new_signed_proposal(
        Height::new(1),
        Round::new(1),
        value.clone(),
        Round::Nil,
        my_addr,
    );
    let outputs = driver
        .process(Input::Proposal(proposal, Validity::Valid))
        .unwrap();
    assert!(
        outputs.is_empty(),
        "Finalize step should reject Proposal input"
    );

    let outputs = driver
        .process(Input::NewRound(Height::new(1), Round::new(1), my_addr))
        .unwrap();
    assert!(
        outputs.is_empty(),
        "Finalize step should reject NewRound input"
    );
}

#[test]
fn cannot_transition_to_finalize_from_other_steps() {
    let value = Value::new(9999);
    let [(v1, sk1), (v2, _sk2), (v3, _sk3)] = make_validators([1, 2, 3]);
    let (_my_sk, my_addr) = (sk1, v1.address);

    let height = Height::new(1);
    let ctx = TestContext::new();
    let vs = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    // Test from Propose step
    {
        let mut driver = Driver::new(ctx.clone(), height, vs.clone(), my_addr, Default::default());

        let _ = driver.process(Input::NewRound(Height::new(1), Round::new(0), my_addr));
        assert_eq!(driver.round_state().step, Step::Propose);

        let outputs = driver.process(Input::TransitionToFinalize).unwrap();
        assert!(
            outputs.is_empty(),
            "Cannot transition to Finalize from Propose step"
        );
        assert_eq!(
            driver.round_state().step,
            Step::Propose,
            "Should remain in Propose step"
        );
    }

    // Test from Prevote step
    {
        let mut driver = Driver::new(ctx.clone(), height, vs.clone(), my_addr, Default::default());

        let proposal = new_signed_proposal(
            Height::new(1),
            Round::new(0),
            value.clone(),
            Round::Nil,
            my_addr,
        );

        let _ = driver.process(Input::NewRound(Height::new(1), Round::new(0), my_addr));
        let _ = driver.process(Input::Proposal(proposal, Validity::Valid));
        assert_eq!(driver.round_state().step, Step::Prevote);

        let outputs = driver.process(Input::TransitionToFinalize).unwrap();
        assert!(
            outputs.is_empty(),
            "Cannot transition to Finalize from Prevote step"
        );
        assert_eq!(
            driver.round_state().step,
            Step::Prevote,
            "Should remain in Prevote step"
        );
    }

    // Test from Precommit step
    {
        let mut driver = Driver::new(ctx.clone(), height, vs.clone(), my_addr, Default::default());

        let proposal = new_signed_proposal(
            Height::new(1),
            Round::new(0),
            value.clone(),
            Round::Nil,
            my_addr,
        );

        let _ = driver.process(Input::NewRound(Height::new(1), Round::new(0), my_addr));
        let _ = driver.process(Input::Proposal(proposal, Validity::Valid));
        let _ = driver.process(Input::Vote(new_signed_prevote(
            Height::new(1),
            Round::new(0),
            NilOrVal::Val(value.id()),
            v2.address,
        )));
        let _ = driver.process(Input::Vote(new_signed_prevote(
            Height::new(1),
            Round::new(0),
            NilOrVal::Val(value.id()),
            v3.address,
        )));
        assert_eq!(driver.round_state().step, Step::Precommit);

        let outputs = driver.process(Input::TransitionToFinalize).unwrap();
        assert!(
            outputs.is_empty(),
            "Cannot transition to Finalize from Precommit step"
        );
        assert_eq!(
            driver.round_state().step,
            Step::Precommit,
            "Should remain in Precommit step"
        );
    }
}
