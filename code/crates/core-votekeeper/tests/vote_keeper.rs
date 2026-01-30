use informalsystems_malachitebft_core_votekeeper::keeper::{Output, RecordVoteError, VoteKeeper};
use malachitebft_core_types::{NilOrVal, Round, SignedVote};
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

#[test]
fn prevote_apply_nil() {
    let ([addr1, addr2, addr3], mut keeper) = setup([1, 1, 1]);

    let height = Height::new(1);
    let round = Round::new(0);

    let vote = new_signed_prevote(height, round, NilOrVal::Nil, addr1);
    let msg = keeper.apply_vote(vote.clone(), round);
    assert!(matches!(msg, Ok(None)));

    let vote = new_signed_prevote(height, round, NilOrVal::Nil, addr2);
    let msg = keeper.apply_vote(vote.clone(), round);
    assert!(matches!(msg, Ok(None)));

    let vote = new_signed_prevote(height, round, NilOrVal::Nil, addr3);
    let msg = keeper.apply_vote(vote, round);
    assert!(matches!(msg, Ok(Some(Output::PolkaNil))));
}

#[test]
fn precommit_apply_nil() {
    let ([addr1, addr2, addr3], mut keeper) = setup([1, 1, 1]);

    let height = Height::new(1);
    let round = Round::new(0);

    let vote = new_signed_precommit(height, round, NilOrVal::Nil, addr1);
    let msg = keeper.apply_vote(vote.clone(), round);
    assert!(matches!(msg, Ok(None)));

    let vote = new_signed_precommit(height, Round::new(0), NilOrVal::Nil, addr2);
    let msg = keeper.apply_vote(vote.clone(), round);
    assert!(matches!(msg, Ok(None)));

    let vote = new_signed_precommit(height, Round::new(0), NilOrVal::Nil, addr3);
    let msg = keeper.apply_vote(vote, round);
    assert!(matches!(msg, Ok(Some(Output::PrecommitAny))));
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
    assert!(matches!(msg, Ok(None)));

    let vote = new_signed_prevote(height, Round::new(0), val, addr2);
    let msg = keeper.apply_vote(vote.clone(), round);
    assert!(matches!(msg, Ok(None)));

    let vote_nil = new_signed_prevote(height, Round::new(0), NilOrVal::Nil, addr3);
    let msg = keeper.apply_vote(vote_nil, round);
    assert!(matches!(msg, Ok(Some(Output::PolkaAny))));

    let vote = new_signed_prevote(height, Round::new(0), val, addr4);
    let msg = keeper.apply_vote(vote, round);
    assert!(matches!(msg, Ok(Some(Output::PolkaValue(x))) if x == id));
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
    assert!(matches!(msg, Ok(None)));

    let vote = new_signed_precommit(height, Round::new(0), val, addr2);
    let msg = keeper.apply_vote(vote.clone(), round);
    assert!(matches!(msg, Ok(None)));

    let vote_nil = new_signed_precommit(height, Round::new(0), NilOrVal::Nil, addr3);
    let msg = keeper.apply_vote(vote_nil, round);
    assert!(matches!(msg, Ok(Some(Output::PrecommitAny))));

    let vote = new_signed_precommit(height, Round::new(0), val, addr4);
    let msg = keeper.apply_vote(vote, round);
    assert!(matches!(msg, Ok(Some(Output::PrecommitValue(x))) if x == id));
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
    assert!(matches!(msg, Ok(None)));

    let vote = new_signed_prevote(height, fut_round, val, addr2);
    let msg = keeper.apply_vote(vote.clone(), cur_round);
    assert!(matches!(msg, Ok(None)));

    let vote = new_signed_prevote(height, fut_round, val, addr3);
    let msg = keeper.apply_vote(vote, cur_round);
    assert!(matches!(msg, Ok(Some(Output::SkipRound(r))) if r == Round::new(1)));
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
    assert!(matches!(msg, Ok(None)));

    let vote = new_signed_prevote(height, fut_round, val, addr2);
    let msg = keeper.apply_vote(vote.clone(), cur_round);
    assert!(matches!(msg, Ok(None)));

    let vote = new_signed_precommit(height, fut_round, val, addr3);
    let msg = keeper.apply_vote(vote, cur_round);
    assert!(matches!(msg, Ok(Some(Output::SkipRound(r))) if r == Round::new(1)));
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
    assert!(matches!(msg, Ok(None)));

    let vote = new_signed_prevote(height, fut_round, val, addr2);
    let msg = keeper.apply_vote(vote.clone(), cur_round);
    assert!(matches!(msg, Ok(None)));

    let vote = new_signed_precommit(height, fut_round, val, addr3);
    let msg = keeper.apply_vote(vote, cur_round);
    assert!(matches!(msg, Ok(Some(Output::SkipRound(r))) if r == Round::new(1)));
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
    assert!(matches!(msg, Ok(None)));

    let vote = new_signed_prevote(height, fut_round, val, addr2);
    let msg = keeper.apply_vote(vote.clone(), cur_round);
    assert!(matches!(msg, Ok(None)));

    let vote = new_signed_precommit(height, fut_round, val, addr2);
    let msg = keeper.apply_vote(vote, cur_round);
    assert!(matches!(msg, Ok(None)));
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
    assert!(matches!(msg, Ok(None)));

    let vote = new_signed_prevote(height, fut_round, val, addr2);
    let msg = keeper.apply_vote(vote.clone(), cur_round);
    assert!(matches!(msg, Ok(None)));

    let vote = new_signed_precommit(height, fut_round, val, addr2);
    let msg = keeper.apply_vote(vote, cur_round);
    assert!(matches!(msg, Ok(None)));
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
    assert!(matches!(msg, Ok(None)));

    let vote2 = new_signed_prevote(height, round, val, addr1);
    let msg = keeper.apply_vote(vote2.clone(), round);
    assert!(matches!(msg, Ok(None)));

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
    assert!(matches!(msg, Ok(None)));

    let vote12 = new_signed_prevote(height, round, NilOrVal::Nil, addr1);
    let msg = keeper.apply_vote(vote12.clone(), round);
    assert!(matches!(
        msg,
        Err(RecordVoteError::ConflictingVote {
            existing,
            conflicting
        }) if existing == vote11 && conflicting == vote12
    ));

    assert!(!keeper.evidence().is_empty());
    assert_eq!(keeper.evidence().get(&addr1), Some(&vec![(vote11, vote12)]));

    let vote21 = new_signed_prevote(height, round, val1, addr2);
    let msg = keeper.apply_vote(vote21.clone(), round);
    assert!(matches!(msg, Ok(None)));

    let id2 = ValueId::new(2);
    let val2 = NilOrVal::Val(id2);

    let vote22 = new_signed_prevote(height, round, val2, addr2);
    let msg = keeper.apply_vote(vote22.clone(), round);
    assert!(matches!(msg,
        Err(RecordVoteError::ConflictingVote {
            existing,
            conflicting
        }) if existing == vote21 && conflicting == vote22
    ));

    assert_eq!(keeper.evidence().get(&addr2), Some(&vec![(vote21, vote22)]));
}
