use std::collections::HashSet;

use arc_malachitebft_core_consensus::{
    process, Effect, Error, Input, Params, Resumable, Resume, State,
};
use malachitebft_core_types::{
    Context, NilOrVal, Round, RoundCertificate, RoundCertificateType, SignedVote, Timeout,
    ValuePayload, VoteExtensionPolicy,
};
use malachitebft_metrics::Metrics;
use malachitebft_test::utils::validators::make_validators;
use malachitebft_test::{
    Address, Height, Signature, TestContext, Validator, ValidatorSet, ValueId,
};

#[derive(Default)]
struct ActiveTimeouts(HashSet<Timeout>);

impl ActiveTimeouts {
    fn handle(&mut self, effect: Effect<TestContext>) -> Result<Resume<TestContext>, ()> {
        use Effect::*;

        Ok(match effect {
            VerifyRoundCertificate(_, _, _, r) => r.resume_with(Ok(())),
            WalAppend(_, _, r) => r.resume_with(()),
            ScheduleTimeout(timeout, r) => {
                self.0.insert(timeout);
                r.resume_with(())
            }
            CancelTimeout(timeout, r) => {
                self.0.remove(&timeout);
                r.resume_with(())
            }
            CancelAllTimeouts(r) => {
                self.0.clear();
                r.resume_with(())
            }
            _ => Resume::Continue,
        })
    }

    fn contains(&self, timeout: Timeout) -> bool {
        self.0.contains(&timeout)
    }
}

fn make_state(validators: &[Validator], address: Address) -> State<TestContext> {
    State::new(
        TestContext::new(),
        Height::new(1),
        ValidatorSet::new(validators.to_vec()),
        Params {
            address,
            threshold_params: Default::default(),
            value_payload: ValuePayload::ProposalOnly,
            enabled: true,
        },
        1000,
        1000,
    )
}

fn drive(
    state: &mut State<TestContext>,
    metrics: &Metrics,
    timeouts: &mut ActiveTimeouts,
    input: Input<TestContext>,
) {
    let result: Result<(), Error<TestContext>> = process!(
        input: input,
        state: state,
        metrics: metrics,
        with: effect => timeouts.handle(effect)
    );
    result.expect("consensus input should be processed");
}

fn prevote(address: Address, round: Round) -> SignedVote<TestContext> {
    let vote = TestContext::new().new_prevote(
        Height::new(1),
        round,
        NilOrVal::Val(ValueId::new(1)),
        address,
    );
    SignedVote::new(vote, Signature::test())
}

fn precommit(address: Address, round: Round) -> SignedVote<TestContext> {
    let vote = TestContext::new().new_precommit(Height::new(1), round, NilOrVal::Nil, address);
    SignedVote::new(vote, Signature::test())
}

fn non_nil_precommit(address: Address, round: Round) -> SignedVote<TestContext> {
    let vote = TestContext::new().new_precommit(
        Height::new(1),
        round,
        NilOrVal::Val(ValueId::new(1)),
        address,
    );
    SignedVote::new(vote, Signature::test())
}

fn started_state() -> (Vec<Validator>, State<TestContext>, Metrics, ActiveTimeouts) {
    started_state_with_policy(Default::default())
}

fn started_state_with_policy(
    vote_extension_policy: VoteExtensionPolicy,
) -> (Vec<Validator>, State<TestContext>, Metrics, ActiveTimeouts) {
    let validators: Vec<_> = make_validators([25, 25, 25, 25])
        .into_iter()
        .map(|(validator, _)| validator)
        .collect();
    let mut state = make_state(&validators, validators[0].address);
    let metrics = Metrics::new();
    let mut timeouts = ActiveTimeouts::default();

    drive(
        &mut state,
        &metrics,
        &mut timeouts,
        Input::StartHeight(
            Height::new(1),
            ValidatorSet::new(validators.clone()),
            false,
            None,
            vote_extension_policy,
        ),
    );

    (validators, state, metrics, timeouts)
}

#[test]
fn skip_round_certificate_keeps_entered_round_rebroadcast_timeout() {
    let (validators, mut state, metrics, mut timeouts) = started_state();

    let round = Round::new(1);
    drive(
        &mut state,
        &metrics,
        &mut timeouts,
        Input::RoundCertificate(RoundCertificate::new_from_votes(
            Height::new(1),
            round,
            RoundCertificateType::Skip,
            vec![
                prevote(validators[1].address, round),
                prevote(validators[2].address, round),
            ],
        )),
    );

    assert_eq!(state.round(), round);
    assert!(timeouts.contains(Timeout::rebroadcast(round)));
}

#[test]
fn current_round_precommit_certificate_cancels_rebroadcast_timeout() {
    let (validators, mut state, metrics, mut timeouts) = started_state();

    let round = Round::new(0);
    drive(
        &mut state,
        &metrics,
        &mut timeouts,
        Input::RoundCertificate(RoundCertificate::new_from_votes(
            Height::new(1),
            round,
            RoundCertificateType::Precommit,
            vec![
                precommit(validators[0].address, round),
                precommit(validators[1].address, round),
                precommit(validators[2].address, round),
            ],
        )),
    );

    assert_eq!(state.round(), round);
    assert!(!timeouts.contains(Timeout::rebroadcast(round)));
}

#[test]
fn future_round_precommit_certificate_keeps_entered_round_rebroadcast_timeout() {
    let (validators, mut state, metrics, mut timeouts) = started_state();

    let round = Round::new(1);
    drive(
        &mut state,
        &metrics,
        &mut timeouts,
        Input::RoundCertificate(RoundCertificate::new_from_votes(
            Height::new(1),
            round,
            RoundCertificateType::Precommit,
            vec![
                precommit(validators[1].address, round),
                precommit(validators[2].address, round),
                precommit(validators[3].address, round),
            ],
        )),
    );

    assert_eq!(state.round(), round);
    assert!(timeouts.contains(Timeout::rebroadcast(round)));
}

#[test]
fn required_policy_keeps_rebroadcast_when_round_certificate_precommits_are_dropped() {
    let (validators, mut state, metrics, mut timeouts) =
        started_state_with_policy(VoteExtensionPolicy::Required);

    let round = Round::new(0);
    assert!(
        timeouts.contains(Timeout::rebroadcast(round)),
        "StartHeight must schedule a rebroadcast timeout for the current round"
    );

    drive(
        &mut state,
        &metrics,
        &mut timeouts,
        Input::RoundCertificate(RoundCertificate::new_from_votes(
            Height::new(1),
            round,
            RoundCertificateType::Precommit,
            vec![
                non_nil_precommit(validators[1].address, round),
                non_nil_precommit(validators[2].address, round),
                non_nil_precommit(validators[3].address, round),
            ],
        )),
    );

    assert_eq!(state.round(), round);
    assert!(
        timeouts.contains(Timeout::rebroadcast(round)),
        "dropping every rebuilt non-nil precommit must not cancel the current-round rebroadcast"
    );
}
