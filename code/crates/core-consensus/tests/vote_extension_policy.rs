use std::cell::Cell;

use arc_malachitebft_core_consensus::{
    process, Effect, Error, Input, Params, Resumable, Resume, SignedConsensusMsg, State,
};
use bytes::Bytes;
use malachitebft_core_types::{
    NilOrVal, Round, SignedMessage, SignedProposal, SignedVote, ValuePayload, VoteExtensionPolicy,
    VoteType,
};
use malachitebft_metrics::Metrics;
use malachitebft_test::utils::validators::make_validators;
use malachitebft_test::{
    Address, Height, Proposal, Signature, TestContext, Validator, ValidatorSet, Value, ValueId,
    Vote,
};

fn run(r: Result<(), Error<TestContext>>) {
    drop(r);
}

fn make_state(validators: &[Validator], my_addr: Address) -> State<TestContext> {
    let vs = ValidatorSet::new(validators.to_vec());
    State::new(
        TestContext::new(),
        Height::new(1),
        vs,
        Params {
            address: my_addr,
            threshold_params: Default::default(),
            value_payload: ValuePayload::ProposalOnly,
            enabled: true,
        },
        1000,
        500,
    )
}

fn start_height_with_policy(
    state: &mut State<TestContext>,
    metrics: &Metrics,
    validators: &[Validator],
    vote_extension_policy: VoteExtensionPolicy,
    counters: &Counters,
) {
    let vs = ValidatorSet::new(validators.to_vec());

    run(process!(
        input: Input::StartHeight(
            Height::new(1),
            vs,
            false,
            None,
            vote_extension_policy
        ),
        state: state,
        metrics: metrics,
        with: effect => counters.handle(effect)
    ));
}

fn start_required_height(
    state: &mut State<TestContext>,
    metrics: &Metrics,
    validators: &[Validator],
    counters: &Counters,
) {
    start_height_with_policy(
        state,
        metrics,
        validators,
        VoteExtensionPolicy::Required,
        counters,
    );
}

fn start_disabled_height(
    state: &mut State<TestContext>,
    metrics: &Metrics,
    validators: &[Validator],
    counters: &Counters,
) {
    start_height_with_policy(
        state,
        metrics,
        validators,
        VoteExtensionPolicy::Disabled,
        counters,
    );
}

fn signed_prevote(addr: Address, value_id: ValueId) -> SignedVote<TestContext> {
    SignedVote::new(
        Vote::new_prevote(Height::new(1), Round::new(0), NilOrVal::Val(value_id), addr),
        Signature::test(),
    )
}

fn signed_prevote_with_extension(addr: Address, value_id: ValueId) -> SignedVote<TestContext> {
    let mut vote = signed_prevote(addr, value_id);
    vote.message.extension = Some(SignedMessage::new(
        Bytes::from_static(b"malformed-prevote-extension"),
        Signature::test(),
    ));
    vote
}

fn signed_precommit(addr: Address, value_id: ValueId) -> SignedVote<TestContext> {
    SignedVote::new(
        Vote::new_precommit(Height::new(1), Round::new(0), NilOrVal::Val(value_id), addr),
        Signature::test(),
    )
}

fn signed_nil_precommit_with_extension(addr: Address) -> SignedVote<TestContext> {
    let mut vote = SignedVote::new(
        Vote::new_precommit(Height::new(1), Round::new(0), NilOrVal::Nil, addr),
        Signature::test(),
    );
    vote.message.extension = Some(SignedMessage::new(
        Bytes::from_static(b"malformed-nil-precommit-extension"),
        Signature::test(),
    ));
    vote
}

fn signed_precommit_with_extension(addr: Address, value_id: ValueId) -> SignedVote<TestContext> {
    let mut vote = signed_precommit(addr, value_id);
    vote.message.extension = Some(SignedMessage::new(
        Bytes::from_static(b"disabled-height-extension"),
        Signature::test(),
    ));
    vote
}

#[derive(Default)]
struct Counters {
    verify_signature: Cell<u32>,
    verify_vote_extension: Cell<u32>,
    wal_append_vote: Cell<u32>,
    extend_vote: Cell<u32>,
    sign_vote: Cell<u32>,
    published_precommits: Cell<u32>,
}

impl Counters {
    fn reset(&self) {
        self.verify_signature.set(0);
        self.verify_vote_extension.set(0);
        self.wal_append_vote.set(0);
        self.extend_vote.set(0);
        self.sign_vote.set(0);
        self.published_precommits.set(0);
    }

    fn handle(&self, effect: Effect<TestContext>) -> Result<Resume<TestContext>, ()> {
        use Effect::*;

        Ok(match effect {
            VerifySignature(_, _, r) => {
                self.verify_signature.set(self.verify_signature.get() + 1);
                r.resume_with(true)
            }
            VerifyVoteExtension(_, _, _, _, _, _, r) => {
                self.verify_vote_extension
                    .set(self.verify_vote_extension.get() + 1);
                r.resume_with(Ok(()))
            }
            SignVote(vote, r) => {
                self.sign_vote.set(self.sign_vote.get() + 1);
                r.resume_with(SignedVote::new(vote, Signature::test()))
            }
            SignProposal(proposal, r) => {
                r.resume_with(SignedProposal::new(proposal, Signature::test()))
            }
            WalAppend(_, entry, r) => {
                if matches!(entry, Input::Vote(_)) {
                    self.wal_append_vote.set(self.wal_append_vote.get() + 1);
                }
                r.resume_with(())
            }
            PublishConsensusMsg(msg, r) => {
                if let SignedConsensusMsg::Vote(vote) = msg {
                    if vote.message.typ == VoteType::Precommit {
                        self.published_precommits
                            .set(self.published_precommits.get() + 1);
                    }
                }
                r.resume_with(())
            }
            ExtendVote(_, _, _, r) => {
                self.extend_vote.set(self.extend_vote.get() + 1);
                r.resume_with(None)
            }
            VerifyCommitCertificate(_, _, _, r)
            | VerifyPolkaCertificate(_, _, _, r)
            | VerifyRoundCertificate(_, _, _, r) => r.resume_with(Ok(())),
            _ => Resume::Continue,
        })
    }
}

#[test]
fn drops_prevote_with_extension_before_wal_append() {
    let entries: Vec<(Validator, _)> = make_validators([25, 25, 25, 25]).into();
    let validators: Vec<Validator> = entries.into_iter().map(|(v, _)| v).collect();

    let my_addr = validators[0].address;
    let sender_addr = validators[1].address;
    let mut state = make_state(&validators, my_addr);
    let metrics = Metrics::new();
    let counters = Counters::default();

    start_required_height(&mut state, &metrics, &validators, &counters);
    counters.reset();

    let vote = signed_prevote_with_extension(sender_addr, ValueId::new(42));

    run(process!(
        input: Input::Vote(vote),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    ));

    assert_eq!(
        counters.verify_signature.get(),
        1,
        "the vote signature is checked before rejecting the malformed prevote"
    );
    assert_eq!(
        counters.verify_vote_extension.get(),
        0,
        "prevotes must be rejected without app extension verification"
    );
    assert_eq!(
        counters.wal_append_vote.get(),
        0,
        "a prevote with an extension must be dropped before WAL append"
    );
}

#[test]
fn drops_nil_precommit_with_extension_before_wal_append() {
    let entries: Vec<(Validator, _)> = make_validators([25, 25, 25, 25]).into();
    let validators: Vec<Validator> = entries.into_iter().map(|(v, _)| v).collect();

    let my_addr = validators[0].address;
    let sender_addr = validators[1].address;
    let mut state = make_state(&validators, my_addr);
    let metrics = Metrics::new();
    let counters = Counters::default();

    start_required_height(&mut state, &metrics, &validators, &counters);
    counters.reset();

    let vote = signed_nil_precommit_with_extension(sender_addr);

    run(process!(
        input: Input::Vote(vote),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    ));

    assert_eq!(
        counters.verify_signature.get(),
        1,
        "the vote signature is checked before rejecting the malformed nil precommit"
    );
    assert_eq!(
        counters.verify_vote_extension.get(),
        0,
        "nil precommits must be rejected without app extension verification"
    );
    assert_eq!(
        counters.wal_append_vote.get(),
        0,
        "a nil precommit with an extension must be dropped before WAL append"
    );
}

#[test]
fn required_policy_drops_non_nil_precommit_without_extension_before_wal_append() {
    let entries: Vec<(Validator, _)> = make_validators([25, 25, 25, 25]).into();
    let validators: Vec<Validator> = entries.into_iter().map(|(v, _)| v).collect();

    let my_addr = validators[0].address;
    let sender_addr = validators[1].address;
    let mut state = make_state(&validators, my_addr);
    let metrics = Metrics::new();
    let counters = Counters::default();

    start_required_height(&mut state, &metrics, &validators, &counters);
    counters.reset();

    let vote = signed_precommit(sender_addr, ValueId::new(42));

    run(process!(
        input: Input::Vote(vote),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    ));

    assert_eq!(
        counters.verify_signature.get(),
        1,
        "the vote signature is checked before enforcing the extension policy"
    );
    assert_eq!(
        counters.verify_vote_extension.get(),
        0,
        "a missing extension must be rejected without calling app verification"
    );
    assert_eq!(
        counters.wal_append_vote.get(),
        0,
        "a missing required extension must be dropped before WAL append"
    );
}

#[test]
fn required_policy_accepts_non_nil_precommit_with_valid_extension() {
    let entries: Vec<(Validator, _)> = make_validators([25, 25, 25, 25]).into();
    let validators: Vec<Validator> = entries.into_iter().map(|(v, _)| v).collect();

    let my_addr = validators[0].address;
    let sender_addr = validators[1].address;
    let mut state = make_state(&validators, my_addr);
    let metrics = Metrics::new();
    let counters = Counters::default();

    start_required_height(&mut state, &metrics, &validators, &counters);
    counters.reset();

    let vote = signed_precommit_with_extension(sender_addr, ValueId::new(42));

    run(process!(
        input: Input::Vote(vote),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    ));

    assert_eq!(
        counters.verify_signature.get(),
        1,
        "the vote signature is checked before verifying the extension"
    );
    assert_eq!(
        counters.verify_vote_extension.get(),
        1,
        "a required extension must be verified by the app"
    );
    assert_eq!(
        counters.wal_append_vote.get(),
        1,
        "a precommit with a valid required extension must be WAL-appended"
    );
}

#[test]
fn disabled_policy_drops_non_nil_precommit_with_extension_before_app_verification() {
    let entries: Vec<(Validator, _)> = make_validators([25, 25, 25, 25]).into();
    let validators: Vec<Validator> = entries.into_iter().map(|(v, _)| v).collect();

    let my_addr = validators[0].address;
    let sender_addr = validators[1].address;
    let mut state = make_state(&validators, my_addr);
    let metrics = Metrics::new();
    let counters = Counters::default();

    start_disabled_height(&mut state, &metrics, &validators, &counters);
    counters.reset();

    let vote = signed_precommit_with_extension(sender_addr, ValueId::new(42));

    run(process!(
        input: Input::Vote(vote),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    ));

    assert_eq!(
        counters.verify_signature.get(),
        1,
        "the vote signature is checked before enforcing the extension policy"
    );
    assert_eq!(
        counters.verify_vote_extension.get(),
        0,
        "a disabled height must reject extensions without app verification"
    );
    assert_eq!(
        counters.wal_append_vote.get(),
        0,
        "an unexpected extension must be dropped before WAL append"
    );
}

#[test]
fn required_policy_errors_before_signing_local_precommit_without_extension() {
    let entries: Vec<(Validator, _)> = make_validators([25, 25, 25, 25]).into();
    let validators: Vec<Validator> = entries.into_iter().map(|(v, _)| v).collect();

    let proposer = validators[0].address;
    let my_addr = validators[1].address;
    let peer_a = validators[0].address;
    let peer_b = validators[2].address;
    let value = Value::new(42);
    let value_id = value.id();

    let mut state = make_state(&validators, my_addr);
    let metrics = Metrics::new();
    let counters = Counters::default();

    start_required_height(&mut state, &metrics, &validators, &counters);

    let proposal = SignedProposal::new(
        Proposal::new(
            Height::new(1),
            Round::new(0),
            value.clone(),
            Round::Nil,
            proposer,
        ),
        Signature::test(),
    );

    run(process!(
        input: Input::Proposal(proposal),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    ));

    assert!(
        state.last_signed_prevote.is_some(),
        "the proposal should make the local validator prevote for the value"
    );

    counters.reset();

    run(process!(
        input: Input::Vote(signed_prevote(peer_a, value_id)),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    ));

    let result = process!(
        input: Input::Vote(signed_prevote(peer_b, value_id)),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    );

    assert!(matches!(
        result,
        Err(Error::VoteExtensionRequired(height, round, got_value_id))
            if height == Height::new(1)
                && round == Round::new(0)
                && got_value_id == value_id
    ));
    assert_eq!(
        counters.extend_vote.get(),
        1,
        "the local precommit must ask the app for an extension"
    );
    assert_eq!(
        counters.sign_vote.get(),
        0,
        "the local precommit must not be signed without a required extension"
    );
    assert_eq!(
        counters.published_precommits.get(),
        0,
        "the local precommit must not be published without a required extension"
    );
}

#[test]
fn disabled_policy_signs_local_precommit_without_asking_for_extension() {
    let entries: Vec<(Validator, _)> = make_validators([25, 25, 25, 25]).into();
    let validators: Vec<Validator> = entries.into_iter().map(|(v, _)| v).collect();

    let proposer = validators[0].address;
    let my_addr = validators[1].address;
    let peer_a = validators[0].address;
    let peer_b = validators[2].address;
    let value = Value::new(42);
    let value_id = value.id();

    let mut state = make_state(&validators, my_addr);
    let metrics = Metrics::new();
    let counters = Counters::default();

    start_disabled_height(&mut state, &metrics, &validators, &counters);

    let proposal = SignedProposal::new(
        Proposal::new(
            Height::new(1),
            Round::new(0),
            value.clone(),
            Round::Nil,
            proposer,
        ),
        Signature::test(),
    );

    run(process!(
        input: Input::Proposal(proposal),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    ));

    counters.reset();

    run(process!(
        input: Input::Vote(signed_prevote(peer_a, value_id)),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    ));
    run(process!(
        input: Input::Vote(signed_prevote(peer_b, value_id)),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    ));

    assert_eq!(
        counters.extend_vote.get(),
        0,
        "disabled heights must not ask the app for vote extensions"
    );
    assert_eq!(
        counters.sign_vote.get(),
        1,
        "the local precommit should still be signed without an extension"
    );
    assert_eq!(
        counters.published_precommits.get(),
        1,
        "the local precommit should still be published without an extension"
    );
}

#[test]
fn same_height_reset_preserves_required_vote_extension_policy() {
    let entries: Vec<(Validator, _)> = make_validators([25, 25, 25, 25]).into();
    let validators: Vec<Validator> = entries.into_iter().map(|(v, _)| v).collect();

    let my_addr = validators[0].address;
    let mut state = make_state(&validators, my_addr);
    let metrics = Metrics::new();
    let counters = Counters::default();

    start_required_height(&mut state, &metrics, &validators, &counters);
    assert_eq!(state.vote_extension_policy, VoteExtensionPolicy::Required);

    let height = state.height();
    let validator_set = state.validator_set().clone();
    let vote_extension_policy = state.vote_extension_policy;

    state.reset_and_start_height(height, validator_set, None, vote_extension_policy);

    assert_eq!(
        state.vote_extension_policy,
        VoteExtensionPolicy::Required,
        "WAL replay-style same-height resets must keep the current height policy"
    );
}
