use bytes::Bytes;
use malachitebft_core_types::{NilOrVal, Round, SignedExtension, SignedVote, Vote as _, VoteType};

use arc_malachitebft_core_votekeeper::evidence::MAX_EVIDENCE_PER_VALIDATOR;
use arc_malachitebft_core_votekeeper::keeper::{Output, VoteKeeper};

use malachitebft_test::{
    Address, Height, PrivateKey, Signature, TestContext, Validator, ValidatorSet, ValueId, Vote,
};

fn setup<const N: usize>(vp: [u64; N]) -> ([Address; N], VoteKeeper<TestContext>) {
    let mut addrs = [Address::new([0; 20]); N];
    let mut vals = Vec::with_capacity(N);
    for i in 0..N {
        let pk = PrivateKey::from([i as u8; 32]);
        addrs[i] = Address::from_public_key(&pk.public_key());
        vals.push(Validator::new(pk.public_key(), vp[i]));
    }
    let keeper = VoteKeeper::new(ValidatorSet::new(vals), Default::default());
    (addrs, keeper)
}

fn new_signed_prevote(
    height: Height,
    round: Round,
    value: NilOrVal<ValueId>,
    addr: Address,
) -> SignedVote<TestContext> {
    SignedVote::new(
        Vote::new_prevote(height, round, value, addr),
        Signature::test(),
    )
}

fn new_signed_precommit(
    height: Height,
    round: Round,
    value: NilOrVal<ValueId>,
    addr: Address,
) -> SignedVote<TestContext> {
    SignedVote::new(
        Vote::new_precommit(height, round, value, addr),
        Signature::test(),
    )
}

fn new_signed_precommit_with_extension(
    height: Height,
    round: Round,
    value: NilOrVal<ValueId>,
    addr: Address,
    extension: SignedExtension<TestContext>,
) -> SignedVote<TestContext> {
    SignedVote::new(
        Vote::new_precommit(height, round, value, addr).extend(extension),
        Signature::test(),
    )
}

fn test_extension(data: &'static [u8]) -> SignedExtension<TestContext> {
    SignedExtension::new(Bytes::from_static(data), Signature::test())
}

#[test]
fn prevote_apply_nil() {
    let ([addr1, addr2, addr3], mut keeper) = setup([1, 1, 1]);

    let height = Height::new(1);
    let round = Round::new(0);

    let vote = new_signed_prevote(height, round, NilOrVal::Nil, addr1);
    let msg = keeper.apply_vote(vote.clone(), round);
    assert_eq!(msg, None);

    let vote = new_signed_prevote(height, round, NilOrVal::Nil, addr2);
    let msg = keeper.apply_vote(vote.clone(), round);
    assert_eq!(msg, None);

    let vote = new_signed_prevote(height, round, NilOrVal::Nil, addr3);
    let msg = keeper.apply_vote(vote, round);
    assert_eq!(msg, Some(Output::PolkaNil));
}

#[test]
fn precommit_apply_nil() {
    let ([addr1, addr2, addr3], mut keeper) = setup([1, 1, 1]);

    let height = Height::new(1);
    let round = Round::new(0);

    let vote = new_signed_precommit(height, round, NilOrVal::Nil, addr1);
    let msg = keeper.apply_vote(vote.clone(), round);
    assert_eq!(msg, None);

    let vote = new_signed_precommit(height, Round::new(0), NilOrVal::Nil, addr2);
    let msg = keeper.apply_vote(vote.clone(), round);
    assert_eq!(msg, None);

    let vote = new_signed_precommit(height, Round::new(0), NilOrVal::Nil, addr3);
    let msg = keeper.apply_vote(vote, round);
    assert_eq!(msg, Some(Output::PrecommitAny));
}

#[test]
fn prevote_apply_single_value() {
    let ([addr1, addr2, addr3, addr4], mut keeper) = setup([1, 1, 1, 1]);

    let id = ValueId::new(1);
    let val = NilOrVal::Val(id);
    let height = Height::new(1);
    let round = Round::new(0);

    let vote = new_signed_prevote(height, Round::new(0), val, addr1);
    let msg = keeper.apply_vote(vote.clone(), round);
    assert_eq!(msg, None);

    let vote = new_signed_prevote(height, Round::new(0), val, addr2);
    let msg = keeper.apply_vote(vote.clone(), round);
    assert_eq!(msg, None);

    let vote_nil = new_signed_prevote(height, Round::new(0), NilOrVal::Nil, addr3);
    let msg = keeper.apply_vote(vote_nil, round);
    assert_eq!(msg, Some(Output::PolkaAny));

    let vote = new_signed_prevote(height, Round::new(0), val, addr4);
    let msg = keeper.apply_vote(vote, round);
    assert_eq!(msg, Some(Output::PolkaValue(id)));
}

#[test]
fn precommit_apply_single_value() {
    let ([addr1, addr2, addr3, addr4], mut keeper) = setup([1, 1, 1, 1]);

    let id = ValueId::new(1);
    let val = NilOrVal::Val(id);
    let height = Height::new(1);
    let round = Round::new(0);

    let vote = new_signed_precommit(height, Round::new(0), val, addr1);
    let msg = keeper.apply_vote(vote.clone(), round);
    assert_eq!(msg, None);

    // Duplicated
    let vote = new_signed_precommit(height, Round::new(0), val, addr1);
    let msg = keeper.apply_vote(vote.clone(), round);
    assert_eq!(msg, None);

    let vote = new_signed_precommit(height, Round::new(0), val, addr2);
    let msg = keeper.apply_vote(vote.clone(), round);
    assert_eq!(msg, None);

    // Duplicated
    let vote = new_signed_precommit(height, Round::new(0), val, addr2);
    let msg = keeper.apply_vote(vote.clone(), round);
    assert_eq!(msg, None);

    let vote_nil = new_signed_precommit(height, Round::new(0), NilOrVal::Nil, addr3);
    let msg = keeper.apply_vote(vote_nil, round);
    assert_eq!(msg, Some(Output::PrecommitAny));

    let vote = new_signed_precommit(height, Round::new(0), val, addr4);
    let msg = keeper.apply_vote(vote, round);
    assert_eq!(msg, Some(Output::PrecommitValue(id)));

    let per_round = keeper.per_round(round);

    match per_round {
        Some(per_round) => {
            // Build a commit certificate for (round, val)
            let cert = per_round.precommits_for_value(&id);
            assert_eq!(cert.len(), 3);
        }
        None => panic!("Per round not found"),
    }
}

#[test]
fn skip_round_small_quorum_prevotes_two_vals() {
    let ([addr1, addr2, addr3, _], mut keeper) = setup([1, 1, 1, 1]);

    let id = ValueId::new(1);
    let val = NilOrVal::Val(id);
    let height = Height::new(1);
    let cur_round = Round::new(0);
    let fut_round = Round::new(1);

    let vote = new_signed_prevote(height, cur_round, val, addr1);
    let msg = keeper.apply_vote(vote.clone(), cur_round);
    assert_eq!(msg, None);

    let vote = new_signed_prevote(height, fut_round, val, addr2);
    let msg = keeper.apply_vote(vote.clone(), cur_round);
    assert_eq!(msg, None);

    let vote = new_signed_prevote(height, fut_round, val, addr3);
    let msg = keeper.apply_vote(vote, cur_round);
    assert_eq!(msg, Some(Output::SkipRound(Round::new(1))));
}

#[test]
fn skip_round_small_quorum_with_prevote_precommit_two_vals() {
    let ([addr1, addr2, addr3, _], mut keeper) = setup([1, 1, 1, 1]);

    let id = ValueId::new(1);
    let val = NilOrVal::Val(id);
    let height = Height::new(1);
    let cur_round = Round::new(0);
    let fut_round = Round::new(1);

    let vote = new_signed_prevote(height, cur_round, val, addr1);
    let msg = keeper.apply_vote(vote.clone(), cur_round);
    assert_eq!(msg, None);

    let vote = new_signed_prevote(height, fut_round, val, addr2);
    let msg = keeper.apply_vote(vote.clone(), cur_round);
    assert_eq!(msg, None);

    let vote = new_signed_precommit(height, fut_round, val, addr3);
    let msg = keeper.apply_vote(vote, cur_round);
    assert_eq!(msg, Some(Output::SkipRound(Round::new(1))));
}

#[test]
fn skip_round_full_quorum_with_prevote_precommit_two_vals() {
    let ([addr1, addr2, addr3], mut keeper) = setup::<3>([1, 1, 2]);

    let id = ValueId::new(1);
    let val = NilOrVal::Val(id);
    let height = Height::new(1);
    let cur_round = Round::new(0);
    let fut_round = Round::new(1);

    let vote = new_signed_prevote(height, cur_round, val, addr1);
    let msg = keeper.apply_vote(vote.clone(), cur_round);
    assert_eq!(msg, None);

    let vote = new_signed_prevote(height, fut_round, val, addr2);
    let msg = keeper.apply_vote(vote.clone(), cur_round);
    assert_eq!(msg, None);

    let vote = new_signed_precommit(height, fut_round, val, addr3);
    let msg = keeper.apply_vote(vote, cur_round);
    assert_eq!(msg, Some(Output::SkipRound(Round::new(1))));
}

#[test]
fn no_skip_round_small_quorum_with_same_val() {
    let ([addr1, addr2, ..], mut keeper) = setup([1, 1, 1, 1]);

    let id = ValueId::new(1);
    let val = NilOrVal::Val(id);
    let height = Height::new(1);
    let cur_round = Round::new(0);
    let fut_round = Round::new(1);

    let vote = new_signed_prevote(height, cur_round, val, addr1);
    let msg = keeper.apply_vote(vote.clone(), cur_round);
    assert_eq!(msg, None);

    let vote = new_signed_prevote(height, fut_round, val, addr2);
    let msg = keeper.apply_vote(vote.clone(), cur_round);
    assert_eq!(msg, None);

    let vote = new_signed_precommit(height, fut_round, val, addr2);
    let msg = keeper.apply_vote(vote, cur_round);
    assert_eq!(msg, None);
}

#[test]
fn no_skip_round_full_quorum_with_same_val() {
    let ([addr1, addr2, ..], mut keeper) = setup([1, 1, 1, 1]);

    let id = ValueId::new(1);
    let val = NilOrVal::Val(id);
    let height = Height::new(1);
    let cur_round = Round::new(0);
    let fut_round = Round::new(1);

    let vote = new_signed_prevote(height, cur_round, val, addr1);
    let msg = keeper.apply_vote(vote.clone(), cur_round);
    assert_eq!(msg, None);

    let vote = new_signed_prevote(height, fut_round, val, addr2);
    let msg = keeper.apply_vote(vote.clone(), cur_round);
    assert_eq!(msg, None);

    let vote = new_signed_precommit(height, fut_round, val, addr2);
    let msg = keeper.apply_vote(vote, cur_round);
    assert_eq!(msg, None);
}

#[test]
fn skip_round_and_precommit_value_future_round() {
    let ([addr1, addr2, ..], mut keeper) = setup([2, 3, 2]);

    let id = ValueId::new(1);
    let val = NilOrVal::Val(id);
    let height = Height::new(1);
    let cur_round = Round::new(0);
    let fut_round = Round::new(1);

    let vote = new_signed_precommit(height, fut_round, val, addr1);
    let msg = keeper.apply_vote(vote, cur_round);
    assert_eq!(msg, None);

    let vote = new_signed_precommit(height, fut_round, val, addr2);
    let msg = keeper.apply_vote(vote, cur_round);
    assert_eq!(msg, Some(Output::PrecommitValue(id)));
}

#[test]
fn same_votes() {
    let ([addr1, ..], mut keeper) = setup([1, 1]);

    let height = Height::new(1);
    let round = Round::new(0);

    let id = ValueId::new(1);
    let val = NilOrVal::Val(id);

    let vote1 = new_signed_prevote(height, round, val, addr1);
    let msg = keeper.apply_vote(vote1.clone(), round);
    assert_eq!(msg, None);

    let vote2 = new_signed_prevote(height, round, val, addr1);
    let msg = keeper.apply_vote(vote2.clone(), round);
    assert_eq!(msg, None);

    assert!(keeper.evidence().is_empty());
    assert_eq!(keeper.evidence().get(&addr1), None);
}

#[test]
fn equivocation() {
    let ([addr1, addr2, ..], mut keeper) = setup([1, 1, 1]);

    let height = Height::new(1);
    let round = Round::new(0);

    let id1 = ValueId::new(1);
    let val1 = NilOrVal::Val(id1);

    let vote11 = new_signed_prevote(height, round, val1, addr1);
    let msg = keeper.apply_vote(vote11.clone(), round);
    assert_eq!(msg, None);

    let vote12 = new_signed_prevote(height, round, NilOrVal::Nil, addr1);
    let msg = keeper.apply_vote(vote12.clone(), round);
    assert_eq!(msg, None);

    assert!(!keeper.evidence().is_empty());
    assert_eq!(keeper.evidence().get(&addr1), Some(&vec![(vote11, vote12)]));

    let vote21 = new_signed_prevote(height, round, val1, addr2);
    let msg = keeper.apply_vote(vote21.clone(), round);
    assert_eq!(msg, None);

    let id2 = ValueId::new(2);
    let val2 = NilOrVal::Val(id2);

    let vote22 = new_signed_prevote(height, round, val2, addr2);
    let msg = keeper.apply_vote(vote22.clone(), round);
    assert_eq!(msg, None);

    assert_eq!(keeper.evidence().get(&addr2), Some(&vec![(vote21, vote22)]));
}

#[test]
fn precommit_with_extension_replaces_stored_precommit_without_extension() {
    let ([addr1, _], mut keeper) = setup([1, 1]);

    let height = Height::new(1);
    let round = Round::new(0);
    let id = ValueId::new(1);
    let val = NilOrVal::Val(id);

    let bare = new_signed_precommit(height, round, val, addr1);
    assert_eq!(keeper.apply_vote(bare, round), None);

    let extension = test_extension(b"app-data");
    let extended =
        new_signed_precommit_with_extension(height, round, val, addr1, extension.clone());
    assert_eq!(keeper.apply_vote(extended, round), None);

    let per_round = keeper.per_round(round).expect("per-round entry exists");
    let stored = per_round
        .get_vote(VoteType::Precommit, &addr1)
        .expect("precommit stored for validator");
    assert_eq!(stored.extension(), Some(&extension));
}

#[test]
fn has_vote_true_for_same_value_bare_duplicate() {
    let ([addr1, _], mut keeper) = setup([1, 1]);

    let height = Height::new(1);
    let round = Round::new(0);
    let id = ValueId::new(1);
    let val = NilOrVal::Val(id);

    let vote = new_signed_precommit(height, round, val, addr1);
    keeper.apply_vote(vote.clone(), round);

    assert!(keeper.has_vote(&vote));
}

#[test]
fn has_vote_false_for_extension_upgrade() {
    let ([addr1, _], mut keeper) = setup([1, 1]);

    let height = Height::new(1);
    let round = Round::new(0);
    let id = ValueId::new(1);
    let val = NilOrVal::Val(id);

    let bare = new_signed_precommit(height, round, val, addr1);
    keeper.apply_vote(bare, round);

    let extended =
        new_signed_precommit_with_extension(height, round, val, addr1, test_extension(b"app-data"));
    assert!(!keeper.has_vote(&extended));
}

#[test]
fn has_vote_true_for_bare_duplicate_after_extension_upgrade() {
    let ([addr1, _], mut keeper) = setup([1, 1]);

    let height = Height::new(1);
    let round = Round::new(0);
    let id = ValueId::new(1);
    let val = NilOrVal::Val(id);

    let bare = new_signed_precommit(height, round, val, addr1);
    keeper.apply_vote(bare.clone(), round);

    let extended =
        new_signed_precommit_with_extension(height, round, val, addr1, test_extension(b"app-data"));
    keeper.apply_vote(extended, round);

    assert!(keeper.has_vote(&bare));
}

#[test]
fn has_vote_false_for_equivocating_value() {
    let ([addr1, _], mut keeper) = setup([1, 1]);

    let height = Height::new(1);
    let round = Round::new(0);

    let first = new_signed_precommit(height, round, NilOrVal::Val(ValueId::new(1)), addr1);
    keeper.apply_vote(first, round);

    let conflicting = new_signed_precommit(height, round, NilOrVal::Val(ValueId::new(2)), addr1);
    assert!(!keeper.has_vote(&conflicting));
    assert!(!keeper.is_saturated_conflict(&conflicting));
}

#[test]
fn saturated_conflict_after_evidence_cap() {
    let ([addr1, _], mut keeper) = setup([1, 1]);

    let height = Height::new(1);
    let round = Round::new(0);

    let first = new_signed_precommit(height, round, NilOrVal::Val(ValueId::new(1)), addr1);
    keeper.apply_vote(first, round);

    for i in 0..MAX_EVIDENCE_PER_VALIDATOR {
        let conflicting = new_signed_precommit(
            height,
            round,
            NilOrVal::Val(ValueId::new((i + 2) as u64)),
            addr1,
        );
        assert!(
            !keeper.is_saturated_conflict(&conflicting),
            "conflict {i} should still be recordable"
        );
        keeper.apply_vote(conflicting, round);
    }

    let extra = new_signed_precommit(
        height,
        round,
        NilOrVal::Val(ValueId::new((MAX_EVIDENCE_PER_VALIDATOR + 2) as u64)),
        addr1,
    );
    assert!(keeper.is_saturated_conflict(&extra));
    assert!(!keeper.has_vote(&extra));
}

#[test]
fn has_vote_true_for_signature_variant_duplicate() {
    let ([addr1, _], mut keeper) = setup([1, 1]);

    let height = Height::new(1);
    let round = Round::new(0);
    let val = NilOrVal::Val(ValueId::new(1));

    let vote = Vote::new_prevote(height, round, val, addr1);

    let stored = SignedVote::new(vote.clone(), Signature::from_bytes([1; 64]));
    keeper.apply_vote(stored, round);

    let variant = SignedVote::new(vote, Signature::from_bytes([2; 64]));
    assert!(keeper.has_vote(&variant));
}

#[test]
fn precommit_without_extension_does_not_clear_stored_extension() {
    let ([addr1, _], mut keeper) = setup([1, 1]);

    let height = Height::new(1);
    let round = Round::new(0);
    let id = ValueId::new(1);
    let val = NilOrVal::Val(id);

    let extension = test_extension(b"app-data");
    let extended =
        new_signed_precommit_with_extension(height, round, val, addr1, extension.clone());
    assert_eq!(keeper.apply_vote(extended, round), None);

    let bare = new_signed_precommit(height, round, val, addr1);
    assert_eq!(keeper.apply_vote(bare, round), None);

    let per_round = keeper.per_round(round).expect("per-round entry exists");
    let stored = per_round
        .get_vote(VoteType::Precommit, &addr1)
        .expect("precommit stored for validator");
    assert_eq!(stored.extension(), Some(&extension));
}

#[test]
fn per_round_holds_a_prevote_and_a_precommit_per_validator_without_reallocating() {
    let (addrs, mut keeper) = setup([1, 1, 1]);

    let height = Height::new(1);
    let round = Round::new(0);

    keeper.apply_vote(
        new_signed_prevote(height, round, NilOrVal::Nil, addrs[0]),
        round,
    );

    let reserved = keeper
        .per_round(round)
        .expect("per-round entry exists")
        .received_votes()
        .capacity();
    assert!(reserved >= 2 * addrs.len());

    for addr in addrs {
        keeper.apply_vote(
            new_signed_prevote(height, round, NilOrVal::Nil, addr),
            round,
        );
        keeper.apply_vote(
            new_signed_precommit(height, round, NilOrVal::Nil, addr),
            round,
        );
    }

    let received_votes = keeper
        .per_round(round)
        .expect("per-round entry exists")
        .received_votes();
    assert_eq!(received_votes.len(), 2 * addrs.len());
    assert_eq!(received_votes.capacity(), reserved);
}

#[test]
fn conflicting_vote_is_none_when_the_stored_vote_carries_the_same_value() {
    let ([addr1, _], mut keeper) = setup([1, 1]);

    let height = Height::new(1);
    let round = Round::new(0);
    let val = NilOrVal::Val(ValueId::new(1));

    keeper.apply_vote(new_signed_prevote(height, round, val, addr1), round);

    let same = new_signed_prevote(height, round, val, addr1);
    assert_eq!(keeper.conflicting_vote(&same), None);
}

#[test]
fn conflicting_vote_returns_the_stored_vote_of_the_same_type_for_a_different_value() {
    let ([addr1, _], mut keeper) = setup([1, 1]);

    let height = Height::new(1);
    let round = Round::new(0);
    let val = NilOrVal::Val(ValueId::new(1));

    let stored = new_signed_prevote(height, round, val, addr1);
    keeper.apply_vote(stored.clone(), round);

    let conflicting = new_signed_prevote(height, round, NilOrVal::Nil, addr1);
    assert_eq!(keeper.conflicting_vote(&conflicting), Some(&stored));

    // A precommit is a different vote type, so it does not conflict with the stored prevote.
    let other_type = new_signed_precommit(height, round, NilOrVal::Nil, addr1);
    assert_eq!(keeper.conflicting_vote(&other_type), None);
}

#[test]
fn conflicting_vote_is_none_for_an_unseen_round_or_validator() {
    let ([addr1, addr2], mut keeper) = setup([1, 1]);

    let height = Height::new(1);
    let round = Round::new(0);
    let val = NilOrVal::Val(ValueId::new(1));

    keeper.apply_vote(new_signed_prevote(height, round, val, addr1), round);

    let other_round = new_signed_prevote(height, Round::new(1), NilOrVal::Nil, addr1);
    assert_eq!(keeper.conflicting_vote(&other_round), None);

    let other_validator = new_signed_prevote(height, round, NilOrVal::Nil, addr2);
    assert_eq!(keeper.conflicting_vote(&other_validator), None);
}

#[test]
fn detect_equivocation_records_a_pair_once_and_reports_only_the_first() {
    let ([addr1, _], mut keeper) = setup([1, 1]);

    let height = Height::new(1);
    let round = Round::new(0);
    let val = NilOrVal::Val(ValueId::new(1));

    let stored = new_signed_prevote(height, round, val, addr1);
    keeper.apply_vote(stored.clone(), round);

    let conflicting = new_signed_prevote(height, round, NilOrVal::Nil, addr1);
    assert!(!keeper.has_equivocation_evidence(&conflicting));
    assert!(keeper.can_record_equivocation(&conflicting));

    assert_eq!(
        keeper.detect_equivocation(conflicting.clone()),
        Some((stored, conflicting.clone()))
    );

    // The pair is stored, so a redelivery reports nothing new and needs no verification.
    assert!(keeper.has_equivocation_evidence(&conflicting));
    assert!(!keeper.can_record_equivocation(&conflicting));
    assert_eq!(keeper.detect_equivocation(conflicting), None);
}

#[test]
fn detect_equivocation_is_none_without_a_conflict() {
    let ([addr1, _], mut keeper) = setup([1, 1]);

    let height = Height::new(1);
    let round = Round::new(0);
    let val = NilOrVal::Val(ValueId::new(1));

    keeper.apply_vote(new_signed_prevote(height, round, val, addr1), round);

    let same = new_signed_prevote(height, round, val, addr1);
    assert!(!keeper.can_record_equivocation(&same));
    assert_eq!(keeper.detect_equivocation(same), None);
    assert!(keeper.evidence().is_empty());
}

#[test]
fn a_capped_validator_can_no_longer_record_equivocation() {
    let ([addr1, _], mut keeper) = setup([1, 1]);

    let height = Height::new(1);
    let round = Round::new(0);

    keeper.apply_vote(
        new_signed_prevote(height, round, NilOrVal::Nil, addr1),
        round,
    );

    // Fill the per-validator cap with distinct conflicting values.
    for i in 0..MAX_EVIDENCE_PER_VALIDATOR {
        let value = NilOrVal::Val(ValueId::new(i as u64 + 1));
        let conflicting = new_signed_prevote(height, round, value, addr1);
        assert!(keeper.detect_equivocation(conflicting).is_some());
    }

    // A further distinct conflicting value is not retained, so it never becomes recorded
    // evidence. Reporting it as recordable would have it verified on every redelivery.
    let beyond_cap = new_signed_prevote(
        height,
        round,
        NilOrVal::Val(ValueId::new(MAX_EVIDENCE_PER_VALIDATOR as u64 + 1)),
        addr1,
    );

    assert!(keeper.conflicting_vote(&beyond_cap).is_some());
    assert!(!keeper.has_equivocation_evidence(&beyond_cap));
    assert!(!keeper.can_record_equivocation(&beyond_cap));
    assert_eq!(keeper.detect_equivocation(beyond_cap), None);
    assert_eq!(
        keeper.evidence().get(&addr1).map(Vec::len),
        Some(MAX_EVIDENCE_PER_VALIDATOR)
    );
}
