use malachite_actors::host::ProposedValue;
use malachite_common::{Context, Round, SignedProposal, Validity};
use malachite_consensus::FullProposalKeeper;
use malachite_test::utils::validators::make_validators;
use malachite_test::{Address, Proposal, Value};
use malachite_test::{Height, TestContext};

fn signed_proposal_pol(
    ctx: &TestContext,
    height: Height,
    round: Round,
    value: Value,
    pol_round: Round,
    address: Address,
) -> SignedProposal<TestContext> {
    let proposal1 = Proposal::new(height, round, value, pol_round, address);
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

macro_rules! prop {
    ($co:expr, $a:expr, $r:expr, $v:expr, $vr: expr) => {
        signed_proposal_pol(
            $co,
            Height::new(1),
            Round::new($r),
            Value::new($v),
            Round::new($vr),
            $a,
        )
    };
}

macro_rules! value {
    ( $a:expr, $r:expr, $v:expr, $val: expr) => {
        proposed_value(Height::new(1), Round::new($r), Value::new($v), $val, $a)
    };
}

macro_rules! keeper_prop {
    ( $k:expr, $r:expr, $v:expr) => {
        $k.get_full_proposal(&Height::new(1), Round::new($r), &Value::new($v))
    };
}

#[test]
fn get_full_proposal_single_matching_same_round_valid() {
    let [(v, sk)] = make_validators([3]);
    let a = v.address;
    let c = TestContext::new(sk);
    let mut keeper = FullProposalKeeper::<TestContext>::new();

    // Store proposal and value for 10 at round 0
    let prop10 = prop!(&c, a, 0, 10, -1);
    keeper.store_proposal(prop10.clone());
    keeper.store_value(value!(a, 0, 10, Validity::Valid));

    // Check we have full proposals for 10
    let stored10 = keeper_prop!(keeper, 0, 10);
    assert!(stored10.is_some());
    let full_proposal10 = stored10.unwrap();
    assert_eq!(full_proposal10.proposal, prop10);
    assert_eq!(full_proposal10.validity, Validity::Valid);
}

#[test]
fn get_full_proposal_single_matching_same_round_invalid() {
    let [(v, sk)] = make_validators([3]);
    let a = v.address;
    let c = TestContext::new(sk);
    let mut keeper = FullProposalKeeper::<TestContext>::new();

    // Store proposal and value for 10 at round 0
    let prop10 = prop!(&c, a, 0, 10, -1);
    keeper.store_proposal(prop10.clone());
    keeper.store_value(value!(a, 0, 10, Validity::Invalid));

    // Check we have full proposals for 10
    let stored10 = keeper_prop!(keeper, 0, 10);
    assert!(stored10.is_some());
    let full_proposal10 = stored10.unwrap();
    assert_eq!(full_proposal10.proposal, prop10);
    assert_eq!(full_proposal10.validity, Validity::Invalid);
}

#[test]
fn get_full_proposal_single_not_matching_same_round_valid() {
    let [(v, sk)] = make_validators([3]);
    let a = v.address;
    let c = TestContext::new(sk);
    let mut keeper = FullProposalKeeper::<TestContext>::new();

    // Store proposal for 10 at round 0
    let prop10 = prop!(&c, a, 0, 10, -1);
    keeper.store_proposal(prop10.clone());
    // Store value for 20 at round 0
    keeper.store_value(value!(a, 0, 20, Validity::Valid));

    // Check we have incomplete proposals for both 10 and 20
    assert!(keeper_prop!(keeper, 0, 10).is_none());
    assert!(keeper_prop!(keeper, 0, 20).is_none());
}

#[test]
fn get_full_proposal_multi_same_round() {
    let [(v, sk)] = make_validators([3]);
    let a = v.address;
    let c = TestContext::new(sk);
    let mut keeper = FullProposalKeeper::<TestContext>::new();

    // Store proposal and value for 10 at round 0
    let prop10 = prop!(&c, a, 0, 10, -1);
    keeper.store_proposal(prop10.clone());
    keeper.store_value(value!(a, 0, 10, Validity::Valid));

    // Store proposal and invalid value for 20 at round 0
    let prop20 = prop!(&c, a, 0, 20, -1);
    keeper.store_proposal(prop20.clone());
    keeper.store_value(value!(a, 0, 20, Validity::Invalid));

    // Check we have full proposals for both 10 and 20
    let stored10 = keeper_prop!(keeper, 0, 10);
    assert!(stored10.is_some());
    let full_proposal10 = stored10.unwrap();
    assert_eq!(full_proposal10.proposal, prop10);
    assert_eq!(full_proposal10.validity, Validity::Valid);

    let stored20 = keeper_prop!(keeper, 0, 20);
    assert!(stored20.is_some());
    let full_proposal20 = stored20.unwrap();
    assert_eq!(full_proposal20.proposal, prop20);
    assert_eq!(full_proposal20.validity, Validity::Invalid);
}

#[test]
fn get_full_proposal_multi_one_incomplete() {
    let [(v, sk)] = make_validators([3]);
    let a = v.address;
    let c = TestContext::new(sk);
    let mut keeper = FullProposalKeeper::<TestContext>::new();

    // Store proposal for 10 at round 0
    let prop10 = prop!(&c, a, 0, 10, -1);
    keeper.store_proposal(prop10.clone());

    // Store proposal and value for 20 at round 0
    let prop20 = prop!(&c, a, 0, 20, -1);
    keeper.store_proposal(prop20.clone());
    keeper.store_value(value!(a, 0, 20, Validity::Valid));

    let stored10 = keeper_prop!(keeper, 0, 10);
    assert!(stored10.is_none());

    let stored20 = keeper_prop!(keeper, 0, 20);
    assert!(stored20.is_some());
    let full_proposal20 = stored20.unwrap();
    assert_eq!(full_proposal20.proposal, prop20);
    assert_eq!(full_proposal20.validity, Validity::Valid);
}

#[test]
fn get_full_proposal_multi_interleaved_same_round() {
    let [(v, sk)] = make_validators([3]);
    let a = v.address;
    let c = TestContext::new(sk);
    let mut keeper = FullProposalKeeper::<TestContext>::new();

    // Store proposal for 10 at round 0
    let prop10 = prop!(&c, a, 0, 10, -1);
    keeper.store_proposal(prop10.clone());

    // Store value 20 at round 0
    keeper.store_value(value!(a, 0, 20, Validity::Valid));

    // Store value 10 at round 0
    keeper.store_value(value!(a, 0, 10, Validity::Valid));

    // Store proposal for 20 at round 0
    let prop20 = prop!(&c, a, 0, 20, -1);
    keeper.store_proposal(prop20.clone());

    let stored10 = keeper_prop!(keeper, 0, 10);
    assert!(stored10.is_some());
    let full_proposal10 = stored10.unwrap();
    assert_eq!(full_proposal10.proposal, prop10);
    assert_eq!(full_proposal10.validity, Validity::Valid);

    let stored20 = keeper_prop!(keeper, 0, 20);
    assert!(stored20.is_some());
    let full_proposal20 = stored20.unwrap();
    assert_eq!(full_proposal20.proposal, prop20);
    assert_eq!(full_proposal20.validity, Validity::Valid);
}

#[test]
fn get_full_proposal_single_matching_pol_round() {
    let [(v1, sk1), (v2, sk2)] = make_validators([1, 1]);
    let a1 = v1.address;
    let c1 = TestContext::new(sk1);
    let a2 = v2.address;
    let c2 = TestContext::new(sk2);
    let mut keeper = FullProposalKeeper::<TestContext>::new();

    // Store proposal and value 10 at round 0
    keeper.store_proposal(prop!(&c1, a1, 0, 10, -1));
    keeper.store_value(value!(a1, 0, 10, Validity::Valid));

    // Store proposal for 10 at round 1 with pol_round=0
    keeper.store_proposal(prop!(&c2, a2, 1, 10, 0));
    // Should find a full value for 10 at round 1
    assert!(keeper_prop!(keeper, 1, 10).is_some());
}

#[test]
fn get_full_proposal_single_non_matching_pol_round() {
    let [(v1, sk1), (v2, sk2)] = make_validators([1, 1]);
    let a1 = v1.address;
    let c1 = TestContext::new(sk1);
    let a2 = v2.address;
    let c2 = TestContext::new(sk2);
    let mut keeper = FullProposalKeeper::<TestContext>::new();

    // Store proposal and value 10 at round 0
    keeper.store_proposal(prop!(&c1, a1, 0, 10, -1));
    keeper.store_value(value!(a1, 0, 10, Validity::Valid));

    // Store proposal for 20 at round 1 with pol_round=0
    keeper.store_proposal(prop!(&c2, a2, 1, 20, 0));
    // Should not have a full value for 20 at round 1
    assert!(keeper_prop!(keeper, 1, 20).is_none());
}

#[test]
fn get_full_proposal_multi_pol_round() {
    let [(v1, sk1), (v2, sk2)] = make_validators([1, 1]);
    let a1 = v1.address;
    let c1 = TestContext::new(sk1);
    let a2 = v2.address;
    let c2 = TestContext::new(sk2);
    let mut keeper = FullProposalKeeper::<TestContext>::new();

    // Store value 10 at round 0
    keeper.store_value(value!(a1, 0, 10, Validity::Valid));

    // Store proposal and value 20 at round 0
    keeper.store_proposal(prop!(&c1, a1, 0, 20, -1));
    keeper.store_value(value!(a1, 0, 20, Validity::Valid));

    // Store proposal for 10 at round 1 with pol_round=0
    keeper.store_proposal(prop!(&c2, a2, 1, 10, 0));
    // Should find a full value for 10 at round 1
    assert!(keeper_prop!(keeper, 1, 10).is_some());

    // Store proposal for 20 at round 1 with pol_round=0
    keeper.store_proposal(prop!(&c2, a2, 1, 20, 0));
    // Should find a full value for 20 at round 1
    assert!(keeper_prop!(keeper, 1, 10).is_some());
}
