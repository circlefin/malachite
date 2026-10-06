//! WAL replay must reuse the extension this node already signed, not ask the
//! application for a fresh one. A second extension for the same precommit can
//! diverge from what peers recorded even though both signatures are valid.

use std::vec::Vec;

use arc_malachitebft_core_consensus::{
    process, Effect, Error, Input, Params, Resumable, Resume, SignedConsensusMsg, State,
    ValuePayload,
};
use bytes::Bytes;
use malachitebft_core_types::{
    NilOrVal, Round, SignedExtension, SignedMessage, SignedProposal, SignedVote, Vote as _,
    VoteExtensionPolicy, VoteType,
};
use malachitebft_metrics::Metrics;
use malachitebft_test::utils::validators::make_validators;
use malachitebft_test::{
    Address, Height, Proposal, Signature, TestContext, Validator, ValidatorSet, Value, ValueId,
    Vote,
};

fn run(r: Result<(), Error<TestContext>>) {
    if let Err(e) = r {
        panic!("consensus step failed: {e}");
    }
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

fn start_required_height(
    state: &mut State<TestContext>,
    metrics: &Metrics,
    validators: &[Validator],
    cap: &mut Captured,
) {
    let vs = ValidatorSet::new(validators.to_vec());
    run(process!(
        input: Input::StartHeight(
            Height::new(1),
            vs,
            false,
            None,
            VoteExtensionPolicy::Required
        ),
        state: state,
        metrics: metrics,
        with: effect => handle_effect(effect, cap)
    ));
}

fn signed_prevote(addr: Address, value_id: ValueId) -> SignedVote<TestContext> {
    SignedVote::new(
        Vote::new_prevote(Height::new(1), Round::new(0), NilOrVal::Val(value_id), addr),
        Signature::test(),
    )
}

fn signed_precommit(addr: Address, value_id: ValueId) -> SignedVote<TestContext> {
    SignedVote::new(
        Vote::new_precommit(Height::new(1), Round::new(0), NilOrVal::Val(value_id), addr),
        Signature::test(),
    )
}

fn extension(data: &'static [u8]) -> SignedExtension<TestContext> {
    SignedMessage::new(Bytes::from_static(data), Signature::test())
}

struct Captured {
    wal: Vec<Input<TestContext>>,
    extend_vote: u32,
    next_extension: SignedExtension<TestContext>,
    published_precommit_extensions: Vec<SignedExtension<TestContext>>,
}

impl Captured {
    fn new(next_extension: SignedExtension<TestContext>) -> Self {
        Self {
            wal: Vec::new(),
            extend_vote: 0,
            next_extension,
            published_precommit_extensions: Vec::new(),
        }
    }
}

fn handle_effect(
    effect: Effect<TestContext>,
    cap: &mut Captured,
) -> Result<Resume<TestContext>, ()> {
    use Effect::*;
    Ok(match effect {
        VerifySignature(_, _, r) => r.resume_with(true),
        VerifyVoteExtension(_, _, _, _, _, _, r) => r.resume_with(Ok(())),
        SignVote(vote, r) => r.resume_with(SignedVote::new(vote, Signature::test())),
        SignProposal(proposal, r) => {
            r.resume_with(SignedProposal::new(proposal, Signature::test()))
        }
        WalAppend(_, entry, r) => {
            cap.wal.push(entry);
            r.resume_with(())
        }
        PublishConsensusMsg(msg, r) => {
            if let SignedConsensusMsg::Vote(vote) = msg {
                if vote.vote_type() == VoteType::Precommit {
                    if let Some(ext) = vote.extension() {
                        cap.published_precommit_extensions.push(ext.clone());
                    }
                }
            }
            r.resume_with(())
        }
        ExtendVote(_, _, _, _, r) => {
            cap.extend_vote += 1;
            r.resume_with(Some(cap.next_extension.clone()))
        }
        VerifyCommitCertificate(_, _, _, r)
        | VerifyPolkaCertificate(_, _, _, r)
        | VerifyRoundCertificate(_, _, _, r) => r.resume_with(Ok(())),
        _ => Resume::Continue,
    })
}

fn drive_to_local_precommit(
    state: &mut State<TestContext>,
    metrics: &Metrics,
    validators: &[Validator],
    my_addr: Address,
    value: &Value,
    cap: &mut Captured,
) {
    let proposer = validators[0].address;
    let peer_a = validators[0].address;
    let peer_b = validators[2].address;
    assert_ne!(proposer, my_addr);
    assert_ne!(peer_b, my_addr);

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
        state: state,
        metrics: metrics,
        with: effect => handle_effect(effect, cap)
    ));
    run(process!(
        input: Input::Vote(signed_prevote(peer_a, value.id())),
        state: state,
        metrics: metrics,
        with: effect => handle_effect(effect, cap)
    ));
    run(process!(
        input: Input::Vote(signed_prevote(peer_b, value.id())),
        state: state,
        metrics: metrics,
        with: effect => handle_effect(effect, cap)
    ));
}

#[test]
fn record_recovered_extensions_keeps_only_own_non_nil_precommits() {
    let entries: Vec<(Validator, _)> = make_validators([25, 25, 25, 25]).into();
    let validators: Vec<Validator> = entries.into_iter().map(|(v, _)| v).collect();
    let my_addr = validators[1].address;
    let other_addr = validators[0].address;
    let own_value = ValueId::new(42);
    let other_value = ValueId::new(43);
    let prevote_value = ValueId::new(44);
    let own_ext = extension(b"own-ext");
    let later_own_ext = extension(b"later-own-ext");

    let mut own_precommit = signed_precommit(my_addr, own_value);
    own_precommit.message.extension = Some(own_ext.clone());

    let mut later_own_precommit = signed_precommit(my_addr, own_value);
    later_own_precommit.message.extension = Some(later_own_ext);

    let mut other_precommit = signed_precommit(other_addr, other_value);
    other_precommit.message.extension = Some(extension(b"other-ext"));

    let mut own_prevote = signed_prevote(my_addr, prevote_value);
    own_prevote.message.extension = Some(extension(b"prevote-ext"));

    let mut nil_precommit = SignedVote::new(
        Vote::new_precommit(Height::new(1), Round::new(0), NilOrVal::Nil, my_addr),
        Signature::test(),
    );
    nil_precommit.message.extension = Some(extension(b"nil-ext"));

    let mut state = make_state(&validators, my_addr);
    let metrics = Metrics::new();
    let mut cap = Captured::new(extension(b"unused"));
    start_required_height(&mut state, &metrics, &validators, &mut cap);

    state.record_recovered_own_precommit_extensions(&[
        Input::Vote(own_prevote),
        Input::Vote(other_precommit),
        Input::Vote(nil_precommit),
        Input::Vote(own_precommit),
        Input::Vote(later_own_precommit),
    ]);

    assert_eq!(
        state.recovered_vote_extension(Round::new(0), &own_value),
        Some(&own_ext),
        "the first logged own extension must be kept"
    );
    assert!(
        state
            .recovered_vote_extension(Round::new(0), &other_value)
            .is_none(),
        "a peer precommit must not be recorded"
    );
    assert!(
        state
            .recovered_vote_extension(Round::new(0), &prevote_value)
            .is_none(),
        "a prevote must not be recorded"
    );
}

#[test]
fn height_reset_clears_recovered_vote_extensions() {
    let entries: Vec<(Validator, _)> = make_validators([25, 25, 25, 25]).into();
    let validators: Vec<Validator> = entries.into_iter().map(|(v, _)| v).collect();
    let my_addr = validators[1].address;
    let value_id = ValueId::new(7);
    let own_ext = extension(b"own-ext");

    let mut own_precommit = signed_precommit(my_addr, value_id);
    own_precommit.message.extension = Some(own_ext);

    let mut state = make_state(&validators, my_addr);
    let metrics = Metrics::new();
    let mut cap = Captured::new(extension(b"unused"));
    start_required_height(&mut state, &metrics, &validators, &mut cap);

    state.record_recovered_own_precommit_extensions(&[Input::Vote(own_precommit)]);
    assert!(state
        .recovered_vote_extension(Round::new(0), &value_id)
        .is_some());

    let vs = ValidatorSet::new(validators.clone());
    state.reset_and_start_height(Height::new(1), vs, None, VoteExtensionPolicy::Required);

    assert!(state
        .recovered_vote_extension(Round::new(0), &value_id)
        .is_none());
}

#[test]
fn wal_replay_reuses_stored_vote_extension() {
    let entries: Vec<(Validator, _)> = make_validators([25, 25, 25, 25]).into();
    let validators: Vec<Validator> = entries.into_iter().map(|(v, _)| v).collect();
    let my_addr = validators[1].address;
    let value = Value::new(42);
    let metrics = Metrics::new();

    let original_ext = extension(b"original-extension");
    let mut live = make_state(&validators, my_addr);
    let mut live_cap = Captured::new(original_ext.clone());
    start_required_height(&mut live, &metrics, &validators, &mut live_cap);
    drive_to_local_precommit(
        &mut live,
        &metrics,
        &validators,
        my_addr,
        &value,
        &mut live_cap,
    );

    assert_eq!(live_cap.extend_vote, 1);
    assert_eq!(
        live_cap.published_precommit_extensions,
        vec![original_ext.clone()]
    );
    assert_eq!(
        live.last_signed_precommit
            .as_ref()
            .and_then(|vote| vote.extension()),
        Some(&original_ext)
    );

    let fresh_ext = extension(b"fresh-extension");
    let replay_metrics = Metrics::new();
    let mut replay = make_state(&validators, my_addr);
    let mut replay_cap = Captured::new(fresh_ext);
    start_required_height(&mut replay, &replay_metrics, &validators, &mut replay_cap);
    replay.record_recovered_own_precommit_extensions(&live_cap.wal);
    replay_cap.extend_vote = 0;

    for entry in live_cap.wal {
        run(process!(
            input: entry,
            state: &mut replay,
            metrics: &replay_metrics,
            with: effect => handle_effect(effect, &mut replay_cap)
        ));
    }

    assert_eq!(
        replay_cap.extend_vote, 0,
        "replay must not ask the application for a new extension"
    );
    assert_eq!(
        replay
            .last_signed_precommit
            .as_ref()
            .and_then(|vote| vote.extension()),
        Some(&original_ext),
        "the recovered precommit must keep the extension that was logged"
    );
    assert_eq!(
        replay_cap.published_precommit_extensions,
        vec![original_ext],
        "republishing during replay must carry the stored extension"
    );
}

#[test]
fn wal_replay_without_recovered_extension_asks_the_application() {
    let entries: Vec<(Validator, _)> = make_validators([25, 25, 25, 25]).into();
    let validators: Vec<Validator> = entries.into_iter().map(|(v, _)| v).collect();
    let my_addr = validators[1].address;
    let value = Value::new(42);
    let metrics = Metrics::new();

    let original_ext = extension(b"original-extension");
    let mut live = make_state(&validators, my_addr);
    let mut live_cap = Captured::new(original_ext);
    start_required_height(&mut live, &metrics, &validators, &mut live_cap);
    drive_to_local_precommit(
        &mut live,
        &metrics,
        &validators,
        my_addr,
        &value,
        &mut live_cap,
    );

    let fresh_ext = extension(b"fresh-extension");
    let replay_metrics = Metrics::new();
    let mut replay = make_state(&validators, my_addr);
    let mut replay_cap = Captured::new(fresh_ext.clone());
    start_required_height(&mut replay, &replay_metrics, &validators, &mut replay_cap);
    replay_cap.extend_vote = 0;

    // Crash after the driver voted but before `WalAppend` of this node's
    // non-nil precommit: the log has the inputs that caused the vote, not
    // the vote itself.
    let replay_entries: Vec<_> = live_cap
        .wal
        .into_iter()
        .filter(|entry| {
            !matches!(
                entry,
                Input::Vote(vote)
                    if vote.validator_address() == &my_addr
                        && vote.vote_type() == VoteType::Precommit
                        && vote.value().is_val()
            )
        })
        .collect();

    for entry in replay_entries {
        run(process!(
            input: entry,
            state: &mut replay,
            metrics: &replay_metrics,
            with: effect => handle_effect(effect, &mut replay_cap)
        ));
    }

    assert_eq!(
        replay_cap.extend_vote, 1,
        "a crash before the vote was logged must still ask the application"
    );
    assert_eq!(
        replay
            .last_signed_precommit
            .as_ref()
            .and_then(|vote| vote.extension()),
        Some(&fresh_ext)
    );
}
