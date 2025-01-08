use eyre::{eyre, Result};

use malachitebft_core_types::{NilOrVal, SignedMessage, SigningProvider};
use malachitebft_test::utils::validators::make_validators;
use malachitebft_test::{Address, Height, Proposal, TestContext, ValidatorSet, Value, Vote};

use crate::effect::resume;
use crate::{
    process, ConsensusMsg, Context, Metrics, Params, Round, SignedConsensusMsg, ThresholdParams,
    Timeout, ValuePayload, ValueToPropose,
};

type State = crate::State<TestContext>;
type Resume = crate::Resume<TestContext>;
type Effect = crate::Effect<TestContext>;
type Input = crate::Input<TestContext>;

#[test]
fn start_height_proposer() -> Result<()> {
    do_start_height(Height::new(1), true)?;
    Ok(())
}

#[test]
fn start_height_non_proposer() -> Result<()> {
    do_start_height(Height::new(1), false)?;
    Ok(())
}

#[test]
fn propose() -> Result<()> {
    let round = Round::new(0);
    let height = Height::new(1);

    let (mut state, ctx, metrics) = do_start_height(height, true)?;

    let validator_set = state.validator_set();
    let proposer = validator_set.get_by_index(0).unwrap();
    let public_key = proposer.public_key;

    let value = Value::new(64);

    let proposal = Proposal::new(height, round, value, Round::Nil, *state.address());
    let signed_proposal = ctx.signing_provider().sign_proposal(proposal.clone());

    let vote = Vote::new_prevote(height, round, NilOrVal::Val(value.id()), proposer.address);
    let signed_vote = ctx.signing_provider().sign_vote(vote.clone());

    let mut handle_effect = expect_effects(vec![
        (
            Effect::CancelTimeout(Timeout::propose(round), resume::Continue),
            Resume::Continue,
        ),
        (
            Effect::SignProposal(proposal.clone(), resume::SignedProposal),
            Resume::SignedProposal(signed_proposal.clone()),
        ),
        (
            Effect::VerifySignature(
                SignedMessage::new(
                    ConsensusMsg::Proposal(proposal.clone()),
                    signed_proposal.signature,
                ),
                public_key,
                resume::SignatureValidity,
            ),
            Resume::SignatureValidity(true),
        ),
        (
            Effect::WalAppendMessage(
                SignedConsensusMsg::Proposal(SignedMessage::new(
                    proposal,
                    signed_proposal.signature,
                )),
                resume::Continue,
            ),
            Resume::Continue,
        ),
        (
            Effect::CancelTimeout(Timeout::propose(round), resume::Continue),
            Resume::Continue,
        ),
        (
            Effect::ScheduleTimeout(Timeout::prevote_time_limit(round), resume::Continue),
            Resume::Continue,
        ),
        (
            Effect::SignVote(vote.clone(), resume::SignedVote),
            Resume::SignedVote(signed_vote.clone()),
        ),
        (
            Effect::VerifySignature(
                SignedMessage::new(ConsensusMsg::Vote(vote.clone()), signed_vote.signature),
                public_key,
                resume::SignatureValidity,
            ),
            Resume::SignatureValidity(true),
        ),
        (
            Effect::WalAppendMessage(
                SignedConsensusMsg::Vote(SignedMessage::new(vote, signed_vote.signature)),
                resume::Continue,
            ),
            Resume::Continue,
        ),
        (
            Effect::Publish(SignedConsensusMsg::Vote(signed_vote), resume::Continue),
            Resume::Continue,
        ),
        (
            Effect::Publish(
                SignedConsensusMsg::Proposal(signed_proposal),
                resume::Continue,
            ),
            Resume::Continue,
        ),
    ]);

    let value_to_propose = ValueToPropose {
        height,
        round,
        valid_round: Round::Nil,
        value,
        extension: None,
    };

    process!(
        input: Input::Propose(value_to_propose),
        state: &mut state,
        metrics: &metrics,
        with: effect => handle_effect(effect)
    )
}

#[test]
fn timeout_elapsed_propose() -> Result<()> {
    let height = Height::new(1);
    let (mut state, _ctx, metrics) = setup(height, true);

    let mut handle_effect = expect_effects(vec![]);

    process!(
        input: Input::TimeoutElapsed(Timeout::propose(Round::new(0))),
        state: &mut state,
        metrics: &metrics,
        with: effect => handle_effect(effect)
    )
}

#[test]
fn timeout_elapsed_prevote() -> Result<()> {
    let height = Height::new(1);
    let (mut state, _ctx, metrics) = setup(height, true);

    let mut handle_effect = expect_effects(vec![]);

    process!(
        input: Input::TimeoutElapsed(Timeout::prevote(Round::new(0))),
        state: &mut state,
        metrics: &metrics,
        with: effect => handle_effect(effect)
    )
}

#[test]
fn timeout_elapsed_precommit() -> Result<()> {
    let height = Height::new(1);
    let (mut state, _ctx, metrics) = setup(height, true);

    let mut handle_effect = expect_effects(vec![]);

    process!(
        input: Input::TimeoutElapsed(Timeout::precommit(Round::new(0))),
        state: &mut state,
        metrics: &metrics,
        with: effect => handle_effect(effect)
    )
}

// #[test]
// fn timeout_elapsed_commit() -> Result<()> {
//     let height = Height::new(1);
//     let (mut state, _ctx, metrics) = setup(height, false);
//
//     let mut handle_effect = expect_effects(vec![]);
//
//     process!(
//         input: Input::TimeoutElapsed(Timeout::commit(Round::new(0))),
//         state: &mut state,
//         metrics: &metrics,
//         with: effect => handle_effect(effect)
//     )
// }

fn do_start_height(height: Height, is_proposer: bool) -> Result<(State, TestContext, Metrics)> {
    let (mut state, ctx, metrics) = setup(height, is_proposer);

    let validator_set = state.validator_set().clone();
    let proposer = validator_set.get_by_index(0).unwrap().address;

    let mut expected = vec![
        (
            Effect::CancelAllTimeouts(resume::Continue),
            Resume::Continue,
        ),
        (Effect::ResetTimeouts(resume::Continue), Resume::Continue),
        (
            Effect::CancelAllTimeouts(resume::Continue),
            Resume::Continue,
        ),
        (
            Effect::StartRound(height, Round::new(0), proposer, resume::Continue),
            Resume::Continue,
        ),
        (
            Effect::ScheduleTimeout(Timeout::propose(Round::new(0)), resume::Continue),
            Resume::Continue,
        ),
    ];

    if is_proposer {
        expected.push((
            Effect::GetValue(
                height,
                Round::new(0),
                Timeout::propose(Round::new(0)),
                resume::Continue,
            ),
            Resume::Continue,
        ));
    }

    let mut handle_effect = expect_effects(expected);

    let result: Result<()> = process!(
        input: Input::StartHeight(height, validator_set),
        state: &mut state,
        metrics: &metrics,
        with: effect => handle_effect(effect)
    );

    result?;

    Ok((state, ctx, metrics))
}

fn setup(height: Height, is_proposer: bool) -> (State, TestContext, Metrics) {
    let (validators, private_keys) = make_validators([1, 1, 1])
        .into_iter()
        .unzip::<_, _, Vec<_>, Vec<_>>();

    let private_key = if is_proposer {
        private_keys[0].clone()
    } else {
        private_keys[1].clone()
    };

    let address = Address::from_public_key(&private_key.public_key());
    let validator_set = ValidatorSet::new(validators);

    let ctx = TestContext::new(private_key);

    let params = Params {
        address,
        initial_height: height,
        initial_validator_set: validator_set.clone(),
        threshold_params: ThresholdParams::default(),
        value_payload: ValuePayload::ProposalAndParts,
    };

    let state = State::new(ctx.clone(), params);
    let metrics = Metrics::default();

    (state, ctx, metrics)
}

fn expect_effects(expected: Vec<(Effect, Resume)>) -> impl FnMut(Effect) -> Result<Resume> {
    let mut expected = expected.into_iter();

    move |effect: Effect| match expected.next() {
        Some((expected, resume)) if expected == effect => Ok(resume),

        Some((expected, _)) => Err(eyre!(
            "unexpected effect: got {effect:?}, expected {expected:?}"
        )),

        None => Err(eyre!("unexpected effect: {effect:?}")),
    }
}
