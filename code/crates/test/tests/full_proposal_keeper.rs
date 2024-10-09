use malachite_actors::host::ProposedValue;
use malachite_common::{Context, Round, SignedProposal, Validity};
use malachite_consensus::{FullProposalKeeper, Input};
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

macro_rules! prop_msg {
    ($co:expr, $a:expr, $r:expr, $v:expr, $vr: expr) => {
        Input::Proposal(signed_proposal_pol(
            $co,
            Height::new(1),
            Round::new($r),
            Value::new($v),
            Round::new($vr),
            $a,
        ))
    };
}

macro_rules! value {
    ( $a:expr, $r:expr, $v:expr, $val: expr) => {
        ProposedValue {
            height: Height::new(1),
            round: Round::new($r),
            validator_address: $a,
            value: Value::new($v),
            validity: $val,
            extension: Default::default(),
        }
    };
}

macro_rules! val_msg {
    ( $a:expr, $r:expr, $v:expr, $val: expr) => {
        Input::ReceivedProposedValue(ProposedValue {
            height: Height::new(1),
            round: Round::new($r),
            value: Value::new($v),
            validity: $val,
            validator_address: $a,
            extension: Default::default(),
        })
    };
}

macro_rules! prop_at_round_and_value {
    ( $k:expr, $r:expr, $v:expr) => {
        $k.full_proposal_at_round_and_value(&Height::new(1), Round::new($r), &Value::new($v).id())
    };
}

macro_rules! props_for_value {
    ( $k:expr, $v:expr) => {
        $k.full_proposals_for_value(&$v)
    };
}

// Used for full proposer keeper testing:
// - input: includes a sequence of value and proposal messages that are applied in order
// - some_fp_for_rv - for each element: full proposal expected for (round, value)
// - none_fp_for_rv - for each element: incomplete proposal expected for (round, value)
// - fps_for_value - full proposals expected for a given ProposedValue
struct Test {
    desc: &'static str,
    input: Vec<Input<TestContext>>,
    some_fp_for_rv: Vec<(i64, u64)>,
    none_fp_for_rv: Vec<(i64, u64)>,
    fps_for_value: (ProposedValue<TestContext>, Vec<SignedProposal<TestContext>>),
}

#[test]
fn full_proposal_keeper_tests() {
    let [(v1, sk1), (v2, sk2)] = make_validators([1, 1]);
    let a1 = v1.address;
    let c1 = TestContext::new(sk1);
    let a2 = v2.address;
    let c2 = TestContext::new(sk2);

    let tests = vec![
        Test {
            desc: "BASIC: prop(0, 10, -1), val(0, 10, valid)",
            input: vec![
                prop_msg!(&c1, a1, 0, 10, -1),
                val_msg!(a1, 0, 10, Validity::Valid),
            ],
            some_fp_for_rv: vec![(0, 10)],
            none_fp_for_rv: vec![],
            fps_for_value: (
                value!(a1, 0, 10, Validity::Valid),
                vec![prop!(&c1, a1, 0, 10, -1)],
            ),
        },
        Test {
            desc: "BASIC: prop(0, 10, -1), val(0, 10, invalid)",
            input: vec![
                prop_msg!(&c1, a1, 0, 10, -1),
                val_msg!(a1, 0, 10, Validity::Invalid),
            ],
            some_fp_for_rv: vec![(0, 10)],
            none_fp_for_rv: vec![],
            fps_for_value: (
                value!(a1, 0, 10, Validity::Invalid),
                vec![prop!(&c1, a1, 0, 10, -1)],
            ),
        },
        Test {
            desc: "BASIC: prop(0, 10, -1), val(0, 20, valid)",
            input: vec![
                prop_msg!(&c1, a1, 0, 10, -1),
                val_msg!(a1, 0, 20, Validity::Valid),
            ],
            some_fp_for_rv: vec![],
            none_fp_for_rv: vec![(0, 10), (0, 20)],
            fps_for_value: (value!(a1, 0, 20, Validity::Valid), vec![]),
        },
        Test {
            desc: "BASIC: prop(0, 10, -1), prop(0, 20, -1), val(0, 20, valid)",
            input: vec![
                prop_msg!(&c1, a1, 0, 10, -1),
                prop_msg!(&c1, a1, 0, 20, -1),
                val_msg!(a1, 0, 20, Validity::Valid),
            ],
            some_fp_for_rv: vec![(0, 20)],
            none_fp_for_rv: vec![(0, 10)],
            fps_for_value: (
                value!(a1, 0, 20, Validity::Valid),
                vec![prop!(&c1, a1, 0, 20, -1)],
            ),
        },
        Test {
            desc: "BASIC: prop(0, 10, -1), val(0, 20, valid), val(0, 10, valid), prop(0, 20, -1)",
            input: vec![
                prop_msg!(&c1, a1, 0, 10, -1),
                val_msg!(a1, 0, 20, Validity::Valid),
                val_msg!(a1, 0, 10, Validity::Valid),
                prop_msg!(&c1, a1, 0, 20, -1),
            ],
            some_fp_for_rv: vec![(0, 10), (0, 20)],
            none_fp_for_rv: vec![],
            fps_for_value: (
                value!(a1, 0, 10, Validity::Valid),
                vec![prop!(&c1, a1, 0, 10, -1)],
            ),
        },
        Test {
            desc: "BASIC: prop(0, 10, -1), val(0, 10, valid), prop(0, 20, -1), val(0, 20, valid)",
            input: vec![
                prop_msg!(&c1, a1, 0, 10, -1),
                val_msg!(a1, 0, 10, Validity::Valid),
                prop_msg!(&c1, a1, 0, 20, -1),
                val_msg!(a1, 0, 20, Validity::Valid),
            ],
            some_fp_for_rv: vec![(0, 10), (0, 20)],
            none_fp_for_rv: vec![],
            fps_for_value: (
                value!(a1, 0, 10, Validity::Valid),
                vec![prop!(&c1, a1, 0, 10, -1)],
            ),
        },
        Test {
            desc: "POL: prop(0, 10, -1), val(0, 10, valid), prop(1, 10, 0)",
            input: vec![
                prop_msg!(&c1, a1, 0, 10, -1),
                val_msg!(a1, 0, 10, Validity::Valid),
                prop_msg!(&c2, a2, 1, 10, 0),
            ],
            some_fp_for_rv: vec![(0, 10), (1, 10)],
            none_fp_for_rv: vec![],
            fps_for_value: (
                value!(a1, 0, 10, Validity::Valid),
                vec![prop!(&c1, a1, 0, 10, -1), prop!(&c2, a2, 1, 10, 0)],
            ),
        },
        Test {
            desc: "POL: prop(1, 10, 0), val(0, 10, valid), prop(0, 10, -1), val(0, 20, valid),",
            input: vec![
                prop_msg!(&c2, a2, 1, 10, 0),
                val_msg!(a1, 0, 10, Validity::Valid),
                prop_msg!(&c1, a1, 0, 10, -1),
                val_msg!(a1, 0, 20, Validity::Valid),
            ],
            some_fp_for_rv: vec![(0, 10), (1, 10)],
            none_fp_for_rv: vec![],
            fps_for_value: (value!(a1, 0, 20, Validity::Valid), vec![]),
        },
        Test {
            desc: "POL: prop(0, 10, -1), val(0, 10, valid), prop(1, 20, 0)",
            input: vec![
                prop_msg!(&c1, a1, 0, 20, -1),
                val_msg!(a1, 0, 10, Validity::Valid),
                prop_msg!(&c2, a2, 1, 20, 0),
            ],
            some_fp_for_rv: vec![],
            none_fp_for_rv: vec![(1, 20)],
            fps_for_value: (value!(a1, 0, 20, Validity::Valid), vec![]),
        },
        Test {
            desc: "POL: val(0, 10, valid), prop(0, 20, -1), val(0, 20, valid), prop(1, 10, 0)",
            input: vec![
                val_msg!(a1, 0, 10, Validity::Valid),
                prop_msg!(&c1, a1, 0, 20, -1),
                val_msg!(a1, 0, 20, Validity::Valid),
                prop_msg!(&c2, a2, 1, 10, 0),
                prop_msg!(&c2, a2, 1, 20, 0),
            ],
            some_fp_for_rv: vec![(0, 20), (1, 10)],
            none_fp_for_rv: vec![],
            fps_for_value: (
                value!(a1, 0, 20, Validity::Valid),
                vec![prop!(&c1, a1, 0, 20, -1), prop!(&c2, a2, 1, 20, 0)],
            ),
        },
        Test {
            desc: "POL: prop(1, 10, 0), prop(0, 10, -1), prop(2, 10, 0), val(0, 10, valid)",
            input: vec![
                prop_msg!(&c1, a1, 1, 10, 0),
                prop_msg!(&c2, a2, 0, 10, -1),
                prop_msg!(&c1, a1, 2, 10, 0),
                val_msg!(a1, 0, 10, Validity::Valid),
            ],
            some_fp_for_rv: vec![(0, 10), (1, 10), (2, 10)],
            none_fp_for_rv: vec![],
            fps_for_value: (
                value!(a1, 0, 10, Validity::Valid),
                vec![
                    prop!(&c2, a2, 0, 10, -1),
                    prop!(&c1, a1, 1, 10, 0),
                    prop!(&c1, a1, 2, 10, 0),
                ],
            ),
        },
    ];

    for s in tests {
        println!("Step: {}", s.desc);
        let mut keeper = FullProposalKeeper::<TestContext>::new();

        for m in s.input {
            match m {
                Input::Proposal(p) => keeper.store_proposal(p),
                Input::ReceivedProposedValue(v) => keeper.store_value(&v),
                _ => continue,
            }
        }
        for (r, v) in s.some_fp_for_rv {
            assert!(prop_at_round_and_value!(keeper, r, v).is_some());
        }
        for (r, v) in s.none_fp_for_rv {
            assert!(prop_at_round_and_value!(keeper, r, v).is_none());
        }
        assert_eq!(
            props_for_value!(keeper, s.fps_for_value.0),
            s.fps_for_value.1
        )
    }
}
