use malachite_actors::host::ProposedValue;
use malachite_common::{Context, Round, SignedProposal, Validity};
use malachite_consensus::FullProposalKeeper;
use malachite_test::utils::validators::make_validators;
use malachite_test::{Address, Proposal, Value};
use malachite_test::{Height, TestContext};

fn signed_proposal(
    ctx: &TestContext,
    height: Height,
    round: Round,
    value: Value,
    address: Address,
) -> SignedProposal<TestContext> {
    let proposal1 = Proposal::new(height, round, value, Round::new(-1), address);
    ctx.sign_proposal(proposal1)
}

fn proposed_value(
    height: Height,
    round: Round,
    value: Value,
    validity: Validity,
    validator_address: Address,
) -> ProposedValue<TestContext> {
    ProposedValue {
        height,
        round,
        validator_address,
        value,
        validity,
    }
}

#[test]
fn get_full_proposal_single_matching_same_round_valid() {
    let [(v, sk)] = make_validators([3]);
    let ctx = TestContext::new(sk);
    let mut keeper = FullProposalKeeper::<TestContext>::new();

    let h = Height::new(1);
    let r0 = Round::new(0);

    let v01 = Value::new(10);
    let sp1 = signed_proposal(&ctx, h, r0, v01, v.address);
    keeper.store_proposal(sp1.clone());
    let pv1 = proposed_value(h, r0, v01, Validity::Valid, v.address);
    keeper.store_value(pv1.clone());

    let stored1 = keeper.get_full_proposal(&h, r0, &v01);
    assert!(stored1.is_some());
    let full_proposal1 = stored1.unwrap();
    assert_eq!(full_proposal1.proposal, sp1);
    assert_eq!(full_proposal1.validity, pv1.validity);
}

#[test]
fn get_full_proposal_single_matching_same_round_invalid() {
    let [(v, sk)] = make_validators([3]);
    let ctx = TestContext::new(sk);
    let mut keeper = FullProposalKeeper::<TestContext>::new();

    let h = Height::new(1);
    let r0 = Round::new(0);

    let v01 = Value::new(10);
    let sp1 = signed_proposal(&ctx, h, r0, v01, v.address);
    keeper.store_proposal(sp1.clone());
    let pv1 = proposed_value(h, r0, v01, Validity::Invalid, v.address);
    keeper.store_value(pv1.clone());

    let stored1 = keeper.get_full_proposal(&h, r0, &v01);
    assert!(stored1.is_some());
    let full_proposal1 = stored1.unwrap();
    assert_eq!(full_proposal1.proposal, sp1);
    assert_eq!(full_proposal1.validity, pv1.validity);
}

#[test]
fn get_full_proposal_single_not_matching_same_round_invalid() {
    let [(v, sk)] = make_validators([3]);
    let ctx = TestContext::new(sk);
    let mut keeper = FullProposalKeeper::<TestContext>::new();

    let h = Height::new(1);
    let r0 = Round::new(0);

    let v01 = Value::new(10);
    let sp1 = signed_proposal(&ctx, h, r0, v01, v.address);
    keeper.store_proposal(sp1.clone());

    let v02 = Value::new(20);
    let pv2 = proposed_value(h, r0, v02, Validity::Valid, v.address);
    keeper.store_value(pv2.clone());

    let stored1 = keeper.get_full_proposal(&h, r0, &v01);
    assert!(stored1.is_none());
}

#[test]
fn get_full_proposal_multi_same_round() {
    let [(v, sk)] = make_validators([3]);
    let ctx = TestContext::new(sk);
    let mut keeper = FullProposalKeeper::<TestContext>::new();

    let h = Height::new(1);
    let r0 = Round::new(0);

    let v01 = Value::new(10);
    let sp1 = signed_proposal(&ctx, h, r0, v01, v.address);
    keeper.store_proposal(sp1.clone());
    let pv1 = proposed_value(h, r0, v01, Validity::Valid, v.address);
    keeper.store_value(pv1.clone());

    let stored1 = keeper.get_full_proposal(&h, r0, &v01);
    assert!(stored1.is_some());
    let full_proposal1 = stored1.unwrap();
    assert_eq!(full_proposal1.proposal, sp1);
    assert_eq!(full_proposal1.validity, pv1.validity);

    let v02 = Value::new(20);
    let sp2 = signed_proposal(&ctx, h, r0, v02, v.address);
    keeper.store_proposal(sp2.clone());
    let pv2 = proposed_value(h, r0, v02, Validity::Invalid, v.address);
    keeper.store_value(pv2.clone());

    let stored2 = keeper.get_full_proposal(&h, r0, &v02);
    assert!(stored2.is_some());
    let full_proposal2 = stored2.unwrap();
    assert_eq!(full_proposal2.proposal, sp2);
    assert_eq!(full_proposal2.validity, pv2.validity);
}

#[test]
fn get_full_proposal_multi_interleaved_same_round() {
    let [(v, sk)] = make_validators([3]);
    let ctx = TestContext::new(sk);
    let mut keeper = FullProposalKeeper::<TestContext>::new();

    let h = Height::new(1);
    let r0 = Round::new(0);

    let v01 = Value::new(10);
    let sp1 = signed_proposal(&ctx, h, r0, v01, v.address);
    keeper.store_proposal(sp1.clone());

    let v02 = Value::new(20);
    let pv2 = proposed_value(h, r0, v02, Validity::Valid, v.address);
    keeper.store_value(pv2.clone());

    let pv1 = proposed_value(h, r0, v01, Validity::Valid, v.address);
    keeper.store_value(pv1.clone());

    let sp2 = signed_proposal(&ctx, h, r0, v02, v.address);
    keeper.store_proposal(sp2.clone());

    let stored1 = keeper.get_full_proposal(&h, r0, &v01);
    assert!(stored1.is_some());
    let full_proposal1 = stored1.unwrap();
    assert_eq!(full_proposal1.proposal, sp1);
    assert_eq!(full_proposal1.validity, pv1.validity);

    let stored2 = keeper.get_full_proposal(&h, r0, &v02);
    assert!(stored2.is_some());
    let full_proposal2 = stored2.unwrap();
    assert_eq!(full_proposal2.proposal, sp2);
    assert_eq!(full_proposal2.validity, pv2.validity);
}
