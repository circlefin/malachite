//! Deterministic test that crash recovery via WAL replay reproduces v3's
//! pre-crash voting behaviour when v3's pre-crash `Prevote(value, r=1)` was
//! justified by a `PolkaCertificate(r=0)` received over the liveness layer.
//!
//! The test drives v3 through the pre-crash scenario, capturing every
//! `WalAppend` effect along the way. It then replays exactly those entries
//! against a fresh state. Then it fires the r=1 propose timer that the engine
//! would re-schedule once it resumes real time after replay.
//!
//! The post-replay assertion is that v3's published-vote set after replay
//! exactly equals its pre-crash published-vote set. Subset in one direction
//! catches equivocation; subset in the other catches a WAL that fails to
//! capture enough state to deterministically reconstruct the pre-crash run.
//! Together they assert recovery is both safe and complete.

use std::vec::Vec;

use arc_malachitebft_core_consensus::{
    process, Effect, Error, Input, Params, ProposedValue, Resumable, Resume, SignedConsensusMsg,
    State, ValuePayload, WalEntry,
};
use malachitebft_core_types::{
    NilOrVal, PolkaCertificate, Round, RoundCertificate, RoundCertificateType, SignedProposal,
    SignedVote, Timeout, Validity, ValueOrigin, VoteType,
};
use malachitebft_metrics::Metrics;
use malachitebft_test::utils::validators::make_validators;
use malachitebft_test::{
    Address, Height, Proposal, Signature, TestContext, Validator, ValidatorSet, Value, ValueId,
    Vote,
};

#[derive(Default)]
struct Captured {
    wal: Vec<WalEntry<TestContext>>,
    published_votes: Vec<SignedVote<TestContext>>,
}

fn handle_effect(
    effect: Effect<TestContext>,
    cap: &mut Captured,
) -> Result<Resume<TestContext>, ()> {
    use Effect::*;
    Ok(match effect {
        VerifySignature(_, _, r) => r.resume_with(true),
        VerifyPolkaCertificate(_, _, _, r) => r.resume_with(Ok(())),
        VerifyRoundCertificate(_, _, _, r) => r.resume_with(Ok(())),
        VerifyCommitCertificate(_, _, _, r) => r.resume_with(Ok(())),
        SignVote(vote, r) => r.resume_with(SignedVote::new(vote, Signature::test())),
        SignProposal(proposal, r) => {
            r.resume_with(SignedProposal::new(proposal, Signature::test()))
        }
        WalAppend(_, entry, r) => {
            cap.wal.push(entry);
            r.resume_with(())
        }
        PublishConsensusMsg(msg, r) => {
            if let SignedConsensusMsg::Vote(v) = &msg {
                cap.published_votes.push(v.clone());
            }
            r.resume_with(())
        }
        ExtendVote(_, _, _, r) => r.resume_with(None),
        VerifyVoteExtension(_, _, _, _, _, r) => r.resume_with(Ok(())),
        _ => Resume::Continue,
    })
}

fn drive(
    state: &mut State<TestContext>,
    inputs: Vec<Input<TestContext>>,
    cap: &mut Captured,
    metrics: &Metrics,
) {
    for input in inputs {
        let _: Result<(), Error<TestContext>> = process!(
            input: input,
            state: state,
            metrics: metrics,
            with: e => handle_effect(e, cap)
        );
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
            value_payload: ValuePayload::ProposalAndParts,
            enabled: true,
        },
        1000,
        1000,
    )
}

fn prevote(addr: Address, round: u32, value_id: NilOrVal<ValueId>) -> SignedVote<TestContext> {
    SignedVote::new(
        Vote::new_prevote(Height::new(1), Round::new(round), value_id, addr),
        Signature::test(),
    )
}

fn precommit(addr: Address, round: u32, value_id: NilOrVal<ValueId>) -> SignedVote<TestContext> {
    SignedVote::new(
        Vote::new_precommit(Height::new(1), Round::new(round), value_id, addr),
        Signature::test(),
    )
}

#[test]
fn restart_with_polka_certificate() {
    let validators: Vec<_> = make_validators([2, 3, 2])
        .into_iter()
        .map(|(v, _)| v)
        .collect();
    let v1 = validators[0].address;
    let v2 = validators[1].address;
    let v3 = validators[2].address;
    let vs = ValidatorSet::new(validators.clone());
    let value = Value::new(0x37a5);
    let value_id = value.id();

    // Ordered consensus inputs to v3 before the crash.
    let inputs: Vec<Input<TestContext>> = vec![
        // Enter h=1, r=0.
        Input::StartHeight(Height::new(1), vs.clone(), false, None),
        // Propose timer at r=0 fires: v3 prevotes nil at r=0.
        Input::TimeoutElapsed(Timeout::propose(Round::new(0))),
        // Peers' Precommit RoundCertificate(r=0, nil) carries 2f+1 precommits
        // and schedules v3's precommit timer at r=0.
        Input::RoundCertificate(RoundCertificate::new_from_votes(
            Height::new(1),
            Round::new(0),
            RoundCertificateType::Precommit,
            vec![
                precommit(v1, 0, NilOrVal::Nil),
                precommit(v2, 0, NilOrVal::Nil),
            ],
        )),
        // Precommit timer at r=0 fires: v3 advances to r=1.
        Input::TimeoutElapsed(Timeout::precommit(Round::new(0))),
        // PolkaCertificate(r=0, value) attached to v2's r=1 re-proposal,
        // received via the liveness layer.
        Input::PolkaCertificate(PolkaCertificate::new(
            Height::new(1),
            Round::new(0),
            value_id,
            vec![
                prevote(v1, 0, NilOrVal::Val(value_id)),
                prevote(v2, 0, NilOrVal::Val(value_id)),
            ],
        )),
        // Re-proposal from v2 (the r=1 proposer) with pol_round=0.
        Input::Proposal(SignedProposal::new(
            Proposal::new(
                Height::new(1),
                Round::new(1),
                value.clone(),
                Round::new(0),
                v2,
            ),
            Signature::test(),
        )),
        // Application supplies the full value (ProposalAndParts mode); this
        // matches the stored proposal and drives v3 to `Prevote(value, r=1)`
        // via L28.
        Input::ProposedValue(
            ProposedValue {
                height: Height::new(1),
                round: Round::new(1),
                valid_round: Round::new(0),
                proposer: v2,
                value: value.clone(),
                validity: Validity::Valid,
            },
            ValueOrigin::Consensus,
        ),
    ];

    // Consensus phase: v3 lives through the pre-crash scenario.
    let metrics = Metrics::new();
    let mut state_normal = make_state(&validators, v3);
    let mut cap_normal = Captured::default();
    drive(&mut state_normal, inputs, &mut cap_normal, &metrics);

    // Sanity: v3 emitted the pre-crash Prevote(value, r=1).
    assert!(
        cap_normal.published_votes.iter().any(|v| {
            v.message.height == Height::new(1)
                && v.message.round == Round::new(1)
                && v.message.typ == VoteType::Prevote
                && matches!(v.message.value, NilOrVal::Val(id) if id == value_id)
        }),
        "v3 should have emitted Prevote(value, r=1) before crash; got {:?}",
        cap_normal.published_votes,
    );

    // Recovery phase: replay the captured WAL entries to a fresh state.
    let mut replay_inputs: Vec<Input<TestContext>> =
        vec![Input::StartHeight(Height::new(1), vs, true, None)];
    for entry in cap_normal.wal.drain(..) {
        replay_inputs.push(match entry {
            WalEntry::ConsensusMsg(SignedConsensusMsg::Vote(v)) => Input::Vote(v),
            WalEntry::ConsensusMsg(SignedConsensusMsg::Proposal(p)) => Input::Proposal(p),
            WalEntry::ProposedValue(pv) => Input::ProposedValue(pv, ValueOrigin::Consensus),
            WalEntry::Timeout(t) => Input::TimeoutElapsed(t),
            WalEntry::PolkaCertificate(c) => Input::PolkaCertificate(c),
        });
    }
    // The propose timer for r=1 was scheduled when v3 entered r=1 during replay.
    replay_inputs.push(Input::TimeoutElapsed(Timeout::propose(Round::new(1))));

    let metrics = Metrics::new();
    let mut state_replay = make_state(&validators, v3);
    let mut cap_replay = Captured::default();
    drive(&mut state_replay, replay_inputs, &mut cap_replay, &metrics);

    // The published-vote sets must be equal: WAL replay deterministically
    // re-derives every pre-crash vote (timeouts are WAL'd, so on replay they
    // re-fire via `Input::TimeoutElapsed` and the state machine reproduces the
    // same outputs).
    assert_eq!(cap_replay.published_votes, cap_normal.published_votes);

    // Guards restored from the WAL.
    assert_eq!(
        state_replay.driver.last_prevote(),
        state_normal.driver.last_prevote(),
    );
    assert_eq!(
        state_replay.driver.last_precommit(),
        state_normal.driver.last_precommit(),
    );
    assert_eq!(
        state_replay.driver.round_state().locked,
        state_normal.driver.round_state().locked,
    );
    assert_eq!(
        state_replay.driver.round_state().valid,
        state_normal.driver.round_state().valid,
    );
}
