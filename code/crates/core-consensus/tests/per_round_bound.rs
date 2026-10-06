//! Per-`(height, round)` bounding for the full-proposal keeper and the WAL.
//!
//! A Byzantine source cannot grow the keeper — and therefore the WAL — without limit by flooding
//! distinct proposals or proposed values for a single `(height, round)`. A consensus proposed
//! value is retained at its source round when that bucket has room, applied only to existing
//! matching entries when the source bucket is full, or retained at a round where it holds a polka
//! certificate. Sync values retain their source round because they carry verified commit
//! certificates. A proposal may exceed its bucket's cap when its value holds a polka certificate
//! at the same round.
//! Flooding does not defeat equivocation accountability: the two retained proposals are still
//! forwarded to the driver, which records the evidence.

use std::vec::Vec;

use arc_malachitebft_core_consensus::full_proposal::MAX_PROPOSALS_PER_ROUND;
use arc_malachitebft_core_consensus::{
    process, Effect, Error, Input, Params, ProposedValue, Resumable, Resume, SignedConsensusMsg,
    State, ValuePayload,
};
use malachitebft_core_driver::proposal_keeper::MAX_EVIDENCE_PER_VALIDATOR;
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
    wal: Vec<Input<TestContext>>,
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
        ExtendVote(_, _, _, _, r) => r.resume_with(None),
        VerifyVoteExtension(_, _, _, _, _, _, r) => r.resume_with(Ok(())),
        WalAppend(_, input, r) => {
            cap.wal.push(input);
            r.resume_with(())
        }
        PublishConsensusMsg(msg, r) => {
            if let SignedConsensusMsg::Vote(vote) = msg {
                cap.published_votes.push(vote);
            }
            r.resume_with(())
        }
        _ => Resume::Continue,
    })
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

/// A polka certificate for `value` at `round`, carrying a prevote from every validator.
fn polka_certificate(
    validators: &[Validator],
    round: u32,
    value: u64,
) -> PolkaCertificate<TestContext> {
    let value_id = Value::new(value).id();
    PolkaCertificate::new(
        Height::new(1),
        Round::new(round),
        value_id,
        validators
            .iter()
            .map(|v| prevote(v.address, round, NilOrVal::Val(value_id)))
            .collect(),
    )
}

fn make_state(
    validators: &[Validator],
    my_addr: Address,
    payload: ValuePayload,
) -> State<TestContext> {
    let vs = ValidatorSet::new(validators.to_vec());
    State::new(
        TestContext::new(),
        Height::new(1),
        vs,
        Params {
            address: my_addr,
            threshold_params: Default::default(),
            value_payload: payload,
            enabled: true,
        },
        1000,
        1000,
    )
}

fn proposed_value(proposer: Address, round: u32, value: u64) -> ProposedValue<TestContext> {
    ProposedValue {
        height: Height::new(1),
        round: Round::new(round),
        valid_round: Round::Nil,
        proposer,
        value: Value::new(value),
        validity: Validity::Valid,
    }
}

fn signed_proposal(proposer: Address, round: u32, value: u64) -> SignedProposal<TestContext> {
    signed_proposal_with_pol_round(proposer, round, value, Round::Nil)
}

fn signed_proposal_with_pol_round(
    proposer: Address,
    round: u32,
    value: u64,
    pol_round: Round,
) -> SignedProposal<TestContext> {
    SignedProposal::new(
        Proposal::new(
            Height::new(1),
            Round::new(round),
            Value::new(value),
            pol_round,
            proposer,
        ),
        Signature::test(),
    )
}

fn proposed_values_in_wal(cap: &Captured) -> usize {
    cap.wal
        .iter()
        .filter(|i| matches!(i, Input::ProposedValue(_, _)))
        .count()
}

fn proposals_in_wal(cap: &Captured) -> Vec<SignedProposal<TestContext>> {
    cap.wal
        .iter()
        .filter_map(|input| match input {
            Input::Proposal(proposal) => Some(proposal.clone()),
            _ => None,
        })
        .collect()
}

fn assert_value_prevote(cap: &Captured, height: Height, round: Round, value: u64) {
    assert!(
        cap.published_votes.iter().any(|vote| {
            vote.message.height == height
                && vote.message.round == round
                && vote.message.typ == VoteType::Prevote
                && matches!(vote.message.value, NilOrVal::Val(id) if id == Value::new(value).id())
        }),
        "expected Prevote(value {value}, round {round}), got {:?}",
        cap.published_votes,
    );
}

fn drive(state: &mut State<TestContext>, inputs: Vec<Input<TestContext>>) -> Captured {
    let metrics = Metrics::new();
    let mut cap = Captured::default();
    drive_into(state, inputs, &metrics, &mut cap);
    cap
}

fn drive_into(
    state: &mut State<TestContext>,
    inputs: Vec<Input<TestContext>>,
    metrics: &Metrics,
    cap: &mut Captured,
) {
    for input in inputs {
        let result: Result<(), Error<TestContext>> = process!(
            input: input,
            state: state,
            metrics: metrics,
            with: e => handle_effect(e, cap)
        );
        result.expect("consensus input should process successfully");
    }
}

#[test]
fn consensus_proposed_values_are_bounded_per_round() {
    let validators: Vec<_> = make_validators([1, 1, 1])
        .into_iter()
        .map(|(v, _)| v)
        .collect();
    let me = validators[0].address;
    let proposer = validators[1].address;
    let mut state = make_state(&validators, me, ValuePayload::ProposalAndParts);

    let mut inputs = vec![Input::StartHeight(
        Height::new(1),
        ValidatorSet::new(validators.clone()),
        false,
        None,
        Default::default(),
    )];
    // Flood distinct values for the same (height, round) from the consensus path.
    for value in 0..(MAX_PROPOSALS_PER_ROUND as u64 + 5) {
        inputs.push(Input::ProposedValue(
            proposed_value(proposer, 0, value),
            ValueOrigin::Consensus,
        ));
    }

    let cap = drive(&mut state, inputs);

    assert_eq!(proposed_values_in_wal(&cap), MAX_PROPOSALS_PER_ROUND);
}

#[derive(Clone, Copy)]
enum ReproposalInputOrder {
    ReproposalValuePolka,
    PolkaValueReproposal,
    ValueReproposalRedeliveryPolka,
}

fn assert_reproposal_emits_value_prevote(order: ReproposalInputOrder) {
    let validators: Vec<_> = make_validators([1, 1, 1, 1])
        .into_iter()
        .map(|(v, _)| v)
        .collect();
    let height = Height::new(1);
    let source_round = Round::new(0);
    let polka_round = Round::new(1);
    let proposal_round = Round::new(2);
    let value = MAX_PROPOSALS_PER_ROUND as u64;
    let proposer_state = make_state(
        &validators,
        validators[0].address,
        ValuePayload::ProposalAndParts,
    );
    let source_proposer = *proposer_state.get_proposer(height, source_round);
    let polka_proposer = *proposer_state.get_proposer(height, polka_round);
    let proposal_proposer = *proposer_state.get_proposer(height, proposal_round);
    let driven_proposers = [source_proposer, polka_proposer, proposal_proposer];
    let me = validators
        .iter()
        .find(|validator| !driven_proposers.contains(&validator.address))
        .expect("a validator that does not propose in the driven rounds")
        .address;
    let mut state = make_state(&validators, me, ValuePayload::ProposalAndParts);
    let round_number = |round: Round| round.as_u32().expect("a defined test round");

    let mut inputs = vec![Input::StartHeight(
        height,
        ValidatorSet::new(validators.clone()),
        false,
        None,
        Default::default(),
    )];
    inputs.extend((0..MAX_PROPOSALS_PER_ROUND as u64).map(|filler_value| {
        Input::Proposal(signed_proposal(
            source_proposer,
            round_number(source_round),
            filler_value,
        ))
    }));
    for round in [source_round, polka_round] {
        inputs.push(Input::TimeoutElapsed(Timeout::propose(round)));
        inputs.push(Input::RoundCertificate(RoundCertificate::new_from_votes(
            height,
            round,
            RoundCertificateType::Precommit,
            validators
                .iter()
                .map(|validator| precommit(validator.address, round_number(round), NilOrVal::Nil))
                .collect(),
        )));
        inputs.push(Input::TimeoutElapsed(Timeout::precommit(round)));
    }

    let metrics = Metrics::new();
    let mut captured = Captured::default();
    match order {
        ReproposalInputOrder::ReproposalValuePolka => {
            inputs.push(Input::Proposal(signed_proposal_with_pol_round(
                proposal_proposer,
                round_number(proposal_round),
                value,
                polka_round,
            )));
            inputs.push(Input::ProposedValue(
                proposed_value(source_proposer, round_number(source_round), value),
                ValueOrigin::Consensus,
            ));
            inputs.push(Input::PolkaCertificate(polka_certificate(
                &validators,
                round_number(polka_round),
                value,
            )));
        }
        ReproposalInputOrder::PolkaValueReproposal => {
            inputs.push(Input::PolkaCertificate(polka_certificate(
                &validators,
                round_number(polka_round),
                value,
            )));
            inputs.push(Input::ProposedValue(
                proposed_value(source_proposer, round_number(source_round), value),
                ValueOrigin::Consensus,
            ));
            inputs.push(Input::Proposal(signed_proposal_with_pol_round(
                proposal_proposer,
                round_number(proposal_round),
                value,
                polka_round,
            )));
        }
        ReproposalInputOrder::ValueReproposalRedeliveryPolka => {
            inputs.push(Input::ProposedValue(
                proposed_value(source_proposer, round_number(source_round), value),
                ValueOrigin::Consensus,
            ));
            drive_into(&mut state, inputs, &metrics, &mut captured);
            assert_eq!(proposed_values_in_wal(&captured), 0);
            inputs = Vec::new();
            inputs.push(Input::Proposal(signed_proposal_with_pol_round(
                proposal_proposer,
                round_number(proposal_round),
                value,
                polka_round,
            )));
            inputs.push(Input::ProposedValue(
                proposed_value(source_proposer, round_number(source_round), value),
                ValueOrigin::Consensus,
            ));
            inputs.push(Input::PolkaCertificate(polka_certificate(
                &validators,
                round_number(polka_round),
                value,
            )));
        }
    }
    inputs.push(Input::TimeoutElapsed(Timeout::propose(proposal_round)));

    drive_into(&mut state, inputs, &metrics, &mut captured);

    assert_eq!(proposed_values_in_wal(&captured), 1);
    let live_full_proposal = state
        .full_proposal_at_round_and_value(&height, proposal_round, &Value::new(value))
        .expect("live full proposal")
        .clone();
    assert!(state.exceeds_per_round_cap(height, source_round, &Value::new(value).id()));
    assert_value_prevote(&captured, height, proposal_round, value);

    let mut replay_inputs = vec![Input::StartHeight(
        height,
        ValidatorSet::new(validators.clone()),
        true,
        None,
        Default::default(),
    )];
    replay_inputs.extend(captured.wal.iter().cloned());
    replay_inputs.push(Input::TimeoutElapsed(Timeout::propose(proposal_round)));

    let mut replay_state = make_state(&validators, me, ValuePayload::ProposalAndParts);
    let replayed = drive(&mut replay_state, replay_inputs);

    let replayed_full_proposal = replay_state
        .full_proposal_at_round_and_value(&height, proposal_round, &Value::new(value))
        .expect("replayed full proposal");
    assert_eq!(
        replayed_full_proposal.builder_value,
        live_full_proposal.builder_value
    );
    assert_eq!(replayed_full_proposal.validity, live_full_proposal.validity);
    assert_eq!(replayed_full_proposal.proposal, live_full_proposal.proposal);
    assert!(replay_state.exceeds_per_round_cap(height, source_round, &Value::new(value).id()));
    assert_value_prevote(&replayed, height, proposal_round, value);
}

#[test]
fn reproposal_then_value_then_polka_emits_value_prevote() {
    assert_reproposal_emits_value_prevote(ReproposalInputOrder::ReproposalValuePolka);
}

#[test]
fn polka_then_value_then_reproposal_emits_value_prevote() {
    assert_reproposal_emits_value_prevote(ReproposalInputOrder::PolkaValueReproposal);
}

#[test]
fn value_before_reproposal_requires_redelivery_for_value_prevote() {
    assert_reproposal_emits_value_prevote(ReproposalInputOrder::ValueReproposalRedeliveryPolka);
}

#[test]
fn sync_proposed_values_bypass_the_bound() {
    let validators: Vec<_> = make_validators([1, 1, 1])
        .into_iter()
        .map(|(v, _)| v)
        .collect();
    let me = validators[0].address;
    let proposer = validators[1].address;
    let mut state = make_state(&validators, me, ValuePayload::ProposalAndParts);

    let mut inputs = vec![Input::StartHeight(
        Height::new(1),
        ValidatorSet::new(validators.clone()),
        false,
        None,
        Default::default(),
    )];
    // Fill the (height, round) bucket from the consensus path.
    for value in 0..(MAX_PROPOSALS_PER_ROUND as u64) {
        inputs.push(Input::ProposedValue(
            proposed_value(proposer, 0, value),
            ValueOrigin::Consensus,
        ));
    }
    // A further distinct value from the sync path must still be persisted.
    inputs.push(Input::ProposedValue(
        proposed_value(proposer, 0, 999),
        ValueOrigin::Sync,
    ));

    let cap = drive(&mut state, inputs);

    assert_eq!(proposed_values_in_wal(&cap), MAX_PROPOSALS_PER_ROUND + 1);
}

#[test]
fn sync_value_stored_beyond_cap_pairs_with_later_same_round_proposal() {
    let validators: Vec<_> = make_validators([1, 1, 1])
        .into_iter()
        .map(|(v, _)| v)
        .collect();
    let height = Height::new(1);
    let source_round = Round::new(0);
    let other_round = Round::new(1);
    let value = MAX_PROPOSALS_PER_ROUND as u64;
    let proposer_state = make_state(
        &validators,
        validators[0].address,
        ValuePayload::ProposalAndParts,
    );
    let source_proposer = *proposer_state.get_proposer(height, source_round);
    let other_proposer = *proposer_state.get_proposer(height, other_round);
    let driven_proposers = [source_proposer, other_proposer];
    let me = validators
        .iter()
        .find(|validator| !driven_proposers.contains(&validator.address))
        .expect("a validator that does not propose in the driven rounds")
        .address;
    let mut state = make_state(&validators, me, ValuePayload::ProposalAndParts);
    let round_number = |round: Round| round.as_u32().expect("a defined test round");

    let mut inputs = vec![Input::StartHeight(
        height,
        ValidatorSet::new(validators.clone()),
        false,
        None,
        Default::default(),
    )];
    inputs.extend((0..MAX_PROPOSALS_PER_ROUND as u64).map(|filler_value| {
        Input::Proposal(signed_proposal(
            source_proposer,
            round_number(source_round),
            filler_value,
        ))
    }));
    inputs.push(Input::Proposal(signed_proposal(
        other_proposer,
        round_number(other_round),
        value,
    )));
    inputs.push(Input::ProposedValue(
        proposed_value(source_proposer, round_number(source_round), value),
        ValueOrigin::Sync,
    ));
    inputs.push(Input::Proposal(signed_proposal(
        source_proposer,
        round_number(source_round),
        value,
    )));

    let captured = drive(&mut state, inputs);

    assert_eq!(proposed_values_in_wal(&captured), 1);
    assert_eq!(
        proposals_in_wal(&captured).len(),
        MAX_PROPOSALS_PER_ROUND + 2
    );
    assert!(state
        .full_proposal_at_round_and_value(&height, source_round, &Value::new(value))
        .is_some());
    assert!(state
        .full_proposal_at_round_and_value(&height, other_round, &Value::new(value))
        .is_some());
}

#[test]
fn proposed_values_with_polkas_at_other_rounds_preserve_source_bound() {
    let validators: Vec<_> = make_validators([1, 1, 1])
        .into_iter()
        .map(|(v, _)| v)
        .collect();
    let me = validators[0].address;
    let proposer = validators[1].address;
    let mut state = make_state(&validators, me, ValuePayload::ProposalAndParts);

    let first_certified = MAX_PROPOSALS_PER_ROUND as u64 + 1;
    let second_certified = first_certified + 1;
    let certified_values = [(1, first_certified), (2, second_certified)];
    let height = Height::new(1);
    let source_round = Round::new(0);
    let mut inputs = vec![Input::StartHeight(
        height,
        ValidatorSet::new(validators.clone()),
        false,
        None,
        Default::default(),
    )];
    // Fill the (height, round) bucket from the consensus path.
    for value in 0..(MAX_PROPOSALS_PER_ROUND as u64) {
        inputs.push(Input::ProposedValue(
            proposed_value(proposer, 0, value),
            ValueOrigin::Consensus,
        ));
    }
    // Leave one slot in each certificate-round bucket so the certified value's placement fills it.
    let mut filler_value = second_certified + 1;
    for (round, _) in certified_values {
        for _ in 1..MAX_PROPOSALS_PER_ROUND {
            inputs.push(Input::ProposedValue(
                proposed_value(proposer, round, filler_value),
                ValueOrigin::Consensus,
            ));
            filler_value += 1;
        }
    }
    // A polka forms for a value the node does not hold at a later round, then the value arrives
    // carrying the round of its original proposal parts.
    for (certificate_round, certified_value) in certified_values {
        inputs.push(Input::PolkaCertificate(polka_certificate(
            &validators,
            certificate_round,
            certified_value,
        )));
        inputs.push(Input::ProposedValue(
            proposed_value(proposer, 0, certified_value),
            ValueOrigin::Consensus,
        ));
    }
    // A further uncertified value is still rejected.
    inputs.push(Input::ProposedValue(
        proposed_value(proposer, 0, 999),
        ValueOrigin::Consensus,
    ));

    let cap = drive(&mut state, inputs);

    assert_eq!(proposed_values_in_wal(&cap), MAX_PROPOSALS_PER_ROUND * 3);
    let fresh_value_id = Value::new(filler_value).id();
    for (certificate_round, certified_value) in certified_values {
        let certificate_round = Round::new(certificate_round);
        let certified_value_id = Value::new(certified_value).id();
        assert!(!state.full_proposal_keeper.would_append_distinct(
            height,
            certificate_round,
            &certified_value_id
        ));
        assert!(state.full_proposal_keeper.would_append_distinct(
            height,
            certificate_round,
            &fresh_value_id
        ));
        assert!(state.exceeds_per_round_cap(height, source_round, &certified_value_id));
    }
}

#[test]
fn proposed_value_uses_deterministic_polka_round_across_certificate_orders() {
    let validators: Vec<_> = make_validators([1, 1, 1])
        .into_iter()
        .map(|(v, _)| v)
        .collect();
    let height = Height::new(1);
    let first_polka_round = Round::new(1);
    let second_polka_round = Round::new(2);
    let target = 10;
    let fresh_value_id = Value::new(11).id();

    for certificate_rounds in [[1, 2], [2, 1]] {
        let proposer = validators[1].address;
        let mut state = make_state(
            &validators,
            validators[0].address,
            ValuePayload::ProposalAndParts,
        );
        let mut inputs = vec![Input::StartHeight(
            height,
            ValidatorSet::new(validators.clone()),
            false,
            None,
            Default::default(),
        )];

        inputs.extend((0..MAX_PROPOSALS_PER_ROUND as u64).map(|value| {
            Input::ProposedValue(proposed_value(proposer, 0, value), ValueOrigin::Consensus)
        }));
        inputs.push(Input::ProposedValue(
            proposed_value(proposer, 1, 8),
            ValueOrigin::Consensus,
        ));
        inputs.push(Input::ProposedValue(
            proposed_value(proposer, 2, 9),
            ValueOrigin::Consensus,
        ));
        inputs.extend(
            certificate_rounds.map(|round| {
                Input::PolkaCertificate(polka_certificate(&validators, round, target))
            }),
        );
        inputs.push(Input::ProposedValue(
            proposed_value(proposer, 0, target),
            ValueOrigin::Consensus,
        ));

        drive(&mut state, inputs);

        assert!(state.full_proposal_keeper.would_append_distinct(
            height,
            first_polka_round,
            &fresh_value_id
        ));
        assert!(!state.full_proposal_keeper.would_append_distinct(
            height,
            second_polka_round,
            &fresh_value_id
        ));
    }
}

#[test]
fn polka_certified_value_uses_full_certificate_round_bucket_after_replay() {
    let validators: Vec<_> = make_validators([1, 1, 1])
        .into_iter()
        .map(|(v, _)| v)
        .collect();
    let height = Height::new(1);
    let source_round = Round::new(0);
    let certificate_round = Round::new(1);
    let proposer = validators[1].address;
    let certified_value = (MAX_PROPOSALS_PER_ROUND * 2) as u64;
    let certified_value_id = Value::new(certified_value).id();
    let fresh_value_id = Value::new(certified_value + 1).id();
    let mut state = make_state(
        &validators,
        validators[0].address,
        ValuePayload::ProposalAndParts,
    );

    let mut setup_inputs = vec![Input::StartHeight(
        height,
        ValidatorSet::new(validators.clone()),
        false,
        None,
        Default::default(),
    )];
    setup_inputs.extend((0..MAX_PROPOSALS_PER_ROUND as u64).map(|value| {
        Input::ProposedValue(proposed_value(proposer, 0, value), ValueOrigin::Consensus)
    }));
    setup_inputs.extend((0..MAX_PROPOSALS_PER_ROUND as u64).map(|offset| {
        Input::ProposedValue(
            proposed_value(proposer, 1, MAX_PROPOSALS_PER_ROUND as u64 + offset),
            ValueOrigin::Consensus,
        )
    }));

    setup_inputs.push(Input::PolkaCertificate(polka_certificate(
        &validators,
        1,
        certified_value,
    )));
    setup_inputs.push(Input::ProposedValue(
        proposed_value(proposer, 0, certified_value),
        ValueOrigin::Consensus,
    ));

    let captured = drive(&mut state, setup_inputs);
    assert_eq!(
        proposed_values_in_wal(&captured),
        MAX_PROPOSALS_PER_ROUND * 2 + 1
    );

    let mut replay_inputs = vec![Input::StartHeight(
        height,
        ValidatorSet::new(validators.clone()),
        true,
        None,
        Default::default(),
    )];
    replay_inputs.extend(captured.wal.iter().cloned());
    let mut replay_state = make_state(
        &validators,
        validators[0].address,
        ValuePayload::ProposalAndParts,
    );
    drive(&mut replay_state, replay_inputs);

    for retained_state in [&state, &replay_state] {
        assert!(!retained_state.full_proposal_keeper.would_append_distinct(
            height,
            certificate_round,
            &certified_value_id
        ));
        assert!(retained_state.full_proposal_keeper.would_append_distinct(
            height,
            certificate_round,
            &fresh_value_id
        ));
        assert!(retained_state.exceeds_per_round_cap(height, source_round, &certified_value_id));
    }
}

#[test]
fn valid_redelivery_updates_matching_full_entry_when_source_round_is_full() {
    let validators: Vec<_> = make_validators([1, 1, 1])
        .into_iter()
        .map(|(v, _)| v)
        .collect();
    let height = Height::new(1);
    let source_round = Round::new(0);
    let proposal_round = Round::new(1);
    let value = MAX_PROPOSALS_PER_ROUND as u64;
    let proposer_state = make_state(
        &validators,
        validators[0].address,
        ValuePayload::ProposalAndParts,
    );
    let source_proposer = *proposer_state.get_proposer(height, source_round);
    let proposal_proposer = *proposer_state.get_proposer(height, proposal_round);
    let driven_proposers = [source_proposer, proposal_proposer];
    let me = validators
        .iter()
        .find(|validator| !driven_proposers.contains(&validator.address))
        .expect("a validator that does not propose in the driven rounds")
        .address;
    let mut state = make_state(&validators, me, ValuePayload::ProposalAndParts);
    let round_number = |round: Round| round.as_u32().expect("a defined test round");
    let mut invalid_value = proposed_value(source_proposer, round_number(source_round), value);
    invalid_value.validity = Validity::Invalid;

    let mut inputs = vec![Input::StartHeight(
        height,
        ValidatorSet::new(validators.clone()),
        false,
        None,
        Default::default(),
    )];
    inputs.extend((0..MAX_PROPOSALS_PER_ROUND as u64).map(|filler_value| {
        Input::Proposal(signed_proposal(
            source_proposer,
            round_number(source_round),
            filler_value,
        ))
    }));
    inputs.push(Input::Proposal(signed_proposal(
        proposal_proposer,
        round_number(proposal_round),
        value,
    )));
    inputs.push(Input::ProposedValue(invalid_value, ValueOrigin::Consensus));
    inputs.push(Input::ProposedValue(
        proposed_value(source_proposer, round_number(source_round), value),
        ValueOrigin::Consensus,
    ));
    inputs.push(Input::TimeoutElapsed(Timeout::propose(source_round)));
    inputs.push(Input::RoundCertificate(RoundCertificate::new_from_votes(
        height,
        source_round,
        RoundCertificateType::Precommit,
        validators
            .iter()
            .map(|validator| {
                precommit(validator.address, round_number(source_round), NilOrVal::Nil)
            })
            .collect(),
    )));
    inputs.push(Input::TimeoutElapsed(Timeout::precommit(source_round)));
    inputs.push(Input::TimeoutElapsed(Timeout::propose(proposal_round)));

    let captured = drive(&mut state, inputs);

    assert_eq!(proposed_values_in_wal(&captured), 2);
    let full_proposal = state
        .full_proposal_at_round_and_value(&height, proposal_round, &Value::new(value))
        .expect("matching proposal should be complete");
    assert_eq!(full_proposal.validity, Validity::Valid);
    assert!(state.exceeds_per_round_cap(height, source_round, &Value::new(value).id()));
    assert_value_prevote(&captured, height, proposal_round, value);
}

#[test]
fn polka_certified_value_is_admitted_when_redelivered_after_the_certificate() {
    let validators: Vec<_> = make_validators([1, 1, 1])
        .into_iter()
        .map(|(v, _)| v)
        .collect();
    let me = validators[0].address;
    let proposer = validators[1].address;
    let mut state = make_state(&validators, me, ValuePayload::ProposalAndParts);

    let certified = MAX_PROPOSALS_PER_ROUND as u64 + 1;
    let mut inputs = vec![Input::StartHeight(
        Height::new(1),
        ValidatorSet::new(validators.clone()),
        false,
        None,
        Default::default(),
    )];
    // Fill the (height, round) bucket from the consensus path.
    for value in 0..(MAX_PROPOSALS_PER_ROUND as u64) {
        inputs.push(Input::ProposedValue(
            proposed_value(proposer, 0, value),
            ValueOrigin::Consensus,
        ));
    }
    // The value arrives ahead of any certificate for it, so the cap rejects it.
    inputs.push(Input::ProposedValue(
        proposed_value(proposer, 0, certified),
        ValueOrigin::Consensus,
    ));

    let cap = drive(&mut state, inputs);

    assert_eq!(proposed_values_in_wal(&cap), MAX_PROPOSALS_PER_ROUND);
    assert!(state
        .get_proposed_value_by_id(Height::new(1), Round::new(0), &Value::new(certified).id())
        .is_none());

    // The certificate is retained once it arrives, so the next delivery of the same value is
    // admitted.
    let cap = drive(
        &mut state,
        vec![
            Input::PolkaCertificate(polka_certificate(&validators, 0, certified)),
            Input::ProposedValue(
                proposed_value(proposer, 0, certified),
                ValueOrigin::Consensus,
            ),
        ],
    );

    assert_eq!(proposed_values_in_wal(&cap), 1);
    assert!(state
        .get_proposed_value_by_id(Height::new(1), Round::new(0), &Value::new(certified).id())
        .is_some());
}

fn assert_polka_certified_proposal_cap_behavior(certificate_round: u32, expected_stored: bool) {
    let validators: Vec<_> = make_validators([1, 1, 1])
        .into_iter()
        .map(|(v, _)| v)
        .collect();

    // Run as a non-proposer so the node does not try to build its own proposal at round 0.
    let proposer = *make_state(
        &validators,
        validators[0].address,
        ValuePayload::ProposalOnly,
    )
    .get_proposer(Height::new(1), Round::new(0));
    let me = validators
        .iter()
        .find(|v| v.address != proposer)
        .expect("a non-proposer validator")
        .address;

    let mut state = make_state(&validators, me, ValuePayload::ProposalOnly);

    let certified = MAX_PROPOSALS_PER_ROUND as u64;
    let mut inputs = vec![Input::StartHeight(
        Height::new(1),
        ValidatorSet::new(validators.clone()),
        false,
        None,
        Default::default(),
    )];
    // Fill the (height, round) bucket with distinct proposals from the round-0 proposer.
    inputs.extend(
        (0..(MAX_PROPOSALS_PER_ROUND as u64))
            .map(|value| Input::Proposal(signed_proposal(proposer, 0, value))),
    );
    inputs.push(Input::PolkaCertificate(polka_certificate(
        &validators,
        certificate_round,
        certified,
    )));
    inputs.push(Input::Proposal(signed_proposal(proposer, 0, certified)));
    // A further uncertified proposal is still rejected.
    inputs.push(Input::Proposal(signed_proposal(proposer, 0, certified + 1)));

    let captured = drive(&mut state, inputs);

    let height = Height::new(1);
    let round = Round::new(0);
    assert_eq!(
        state
            .full_proposal_at_round_and_value(&height, round, &Value::new(certified))
            .is_some(),
        expected_stored
    );
    assert_eq!(
        proposals_in_wal(&captured).len(),
        MAX_PROPOSALS_PER_ROUND + usize::from(expected_stored)
    );
    assert!(state
        .full_proposal_at_round_and_value(&height, round, &Value::new(certified + 1))
        .is_none());
    // The two entries stored before the certificate remain retained.
    assert!(state
        .full_proposal_at_round_and_value(&height, round, &Value::new(0))
        .is_some());
    assert!(state
        .full_proposal_at_round_and_value(&height, round, &Value::new(1))
        .is_some());
}

#[test]
fn polka_certified_proposals_bypass_the_bound() {
    assert_polka_certified_proposal_cap_behavior(0, true);
}

#[test]
fn proposals_with_a_polka_at_another_round_do_not_bypass_the_bound() {
    assert_polka_certified_proposal_cap_behavior(1, false);
}

#[test]
fn flooding_proposals_records_at_most_one_evidence_pair() {
    let validators: Vec<_> = make_validators([1, 1, 1])
        .into_iter()
        .map(|(v, _)| v)
        .collect();
    let vs = ValidatorSet::new(validators.clone());

    // Run as a non-proposer so the node does not try to build its own proposal at round 0.
    let proposer = *make_state(
        &validators,
        validators[0].address,
        ValuePayload::ProposalOnly,
    )
    .get_proposer(Height::new(1), Round::new(0));
    let me = validators
        .iter()
        .find(|v| v.address != proposer)
        .expect("a non-proposer validator")
        .address;

    let mut state = make_state(&validators, me, ValuePayload::ProposalOnly);

    // Enter height 1, then have the round-0 proposer flood distinct proposals. In proposal-only
    // mode each is paired with a synthesized value, becoming a full proposal forwarded to the
    // driver.
    let mut inputs = vec![Input::StartHeight(
        Height::new(1),
        vs,
        false,
        None,
        Default::default(),
    )];
    inputs.extend(
        (0..(MAX_PROPOSALS_PER_ROUND as u64 + 8))
            .map(|value| Input::Proposal(signed_proposal(proposer, 0, value))),
    );
    let _ = drive(&mut state, inputs);

    // Only MAX distinct proposals are retained; the rest are dropped at the pre-gate.
    let height = Height::new(1);
    let round = Round::new(0);
    assert!(state
        .full_proposal_at_round_and_value(&height, round, &Value::new(0))
        .is_some());
    assert!(state
        .full_proposal_at_round_and_value(&height, round, &Value::new(1))
        .is_some());
    assert!(state
        .full_proposal_at_round_and_value(&height, round, &Value::new(2))
        .is_none());

    // The two retained proposals equivocate; the driver records exactly one evidence pair.
    let evidence = state.driver.take_proposal_evidence();
    assert_eq!(
        evidence
            .get(&proposer)
            .map(|pairs| pairs.len())
            .unwrap_or(0),
        1
    );
}

#[test]
fn proposal_evidence_and_wal_are_capped_per_validator() {
    let validators: Vec<_> = make_validators([1, 1, 1])
        .into_iter()
        .map(|(v, _)| v)
        .collect();
    let validator_set = ValidatorSet::new(validators.clone());
    let proposal_round = MAX_EVIDENCE_PER_VALIDATOR as u32 + 3;

    let proposer = *make_state(
        &validators,
        validators[0].address,
        ValuePayload::ProposalOnly,
    )
    .get_proposer(Height::new(1), Round::new(proposal_round));
    let me = validators
        .iter()
        .find(|validator| validator.address != proposer)
        .expect("a non-proposer validator")
        .address;
    let mut state = make_state(&validators, me, ValuePayload::ProposalOnly);

    let first = signed_proposal(proposer, proposal_round, 10);
    let signature_variant =
        SignedProposal::new(first.message.clone(), Signature::from_bytes([1; 64]));
    let mut inputs = vec![
        Input::StartHeight(
            Height::new(1),
            validator_set.clone(),
            false,
            None,
            Default::default(),
        ),
        Input::Proposal(first.clone()),
        Input::Proposal(first),
        Input::Proposal(signature_variant),
    ];
    inputs.extend((0..MAX_EVIDENCE_PER_VALIDATOR + 2).map(|pol_round| {
        Input::Proposal(signed_proposal_with_pol_round(
            proposer,
            proposal_round,
            10,
            Round::new(pol_round as u32),
        ))
    }));

    let captured = drive(&mut state, inputs);
    let wal_proposals = proposals_in_wal(&captured);
    assert_eq!(wal_proposals.len(), 1 + MAX_EVIDENCE_PER_VALIDATOR);

    let retained = state
        .driver
        .take_proposal_evidence()
        .get(&proposer)
        .cloned()
        .expect("proposal evidence");
    assert_eq!(retained.len(), MAX_EVIDENCE_PER_VALIDATOR);

    let mut replay_state = make_state(&validators, me, ValuePayload::ProposalOnly);
    let mut replay_inputs = vec![Input::StartHeight(
        Height::new(1),
        validator_set,
        true,
        None,
        Default::default(),
    )];
    replay_inputs.extend(wal_proposals.into_iter().map(Input::Proposal));
    let _ = drive(&mut replay_state, replay_inputs);

    let replayed = replay_state
        .driver
        .take_proposal_evidence()
        .get(&proposer)
        .cloned()
        .expect("replayed proposal evidence");
    assert_eq!(replayed, retained);
}
