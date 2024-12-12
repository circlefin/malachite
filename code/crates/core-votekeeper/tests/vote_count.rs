#![allow(clippy::bool_assert_comparison)]

use malachite_core_types::{NilOrVal, Round};
use malachite_core_votekeeper::count::VoteCount;
use malachite_core_votekeeper::{Threshold, ThresholdParam};
use malachite_test::{Address, Height, TestContext, ValueId, Vote};

#[test]
fn vote_count_nil() {
    let t = 4;
    let q = ThresholdParam::TWO_F_PLUS_ONE;
    let h = Height::new(1);
    let r = Round::new(0);

    let mut vc = VoteCount::<TestContext>::new();

    let addr1 = Address::new([1; 20]);
    let addr2 = Address::new([2; 20]);
    let addr3 = Address::new([3; 20]);
    let addr4 = Address::new([4; 20]);

    let val1 = ValueId::new(1);
    let val2 = ValueId::new(2);

    assert_eq!(vc.get(&NilOrVal::Nil), 0);
    assert_eq!(vc.get(&NilOrVal::Val(val1)), 0);
    assert_eq!(vc.is_threshold_met(Threshold::Unreached, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Any, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Nil, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(val1), q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(val2), q, t), false);

    let vote1 = Vote::new_prevote(h, r, NilOrVal::Nil, addr1);
    assert_eq!(vc.add(&vote1, 1), 1);
    assert_eq!(vc.get(&NilOrVal::Nil), 1);
    assert_eq!(vc.get(&NilOrVal::Val(val1)), 0);
    assert_eq!(vc.is_threshold_met(Threshold::Unreached, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Any, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Nil, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(val1), q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(val2), q, t), false);

    let vote2 = Vote::new_prevote(h, r, NilOrVal::Nil, addr2);
    assert_eq!(vc.add(&vote2, 1), 2);
    assert_eq!(vc.get(&NilOrVal::Nil), 2);
    assert_eq!(vc.get(&NilOrVal::Val(val1)), 0);
    assert_eq!(vc.is_threshold_met(Threshold::Unreached, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Any, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Nil, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(val1), q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(val2), q, t), false);

    // addr1 votes again, is ignored
    let vote3 = Vote::new_prevote(h, r, NilOrVal::Nil, addr1);
    assert_eq!(vc.add(&vote3, 1), 2);
    assert_eq!(vc.get(&NilOrVal::Nil), 2);
    assert_eq!(vc.get(&NilOrVal::Val(val1)), 0);
    assert_eq!(vc.is_threshold_met(Threshold::Unreached, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Any, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Nil, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(val1), q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(val2), q, t), false);

    let vote4 = Vote::new_prevote(h, r, NilOrVal::Nil, addr3);
    assert_eq!(vc.add(&vote4, 1), 3);
    assert_eq!(vc.get(&NilOrVal::Nil), 3);
    assert_eq!(vc.get(&NilOrVal::Val(val1)), 0);
    assert_eq!(vc.is_threshold_met(Threshold::Unreached, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Any, q, t), true);
    assert_eq!(vc.is_threshold_met(Threshold::Nil, q, t), true);
    assert_eq!(vc.is_threshold_met(Threshold::Value(val1), q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(val2), q, t), false);

    let vote5 = Vote::new_prevote(h, r, NilOrVal::Val(ValueId::new(1)), addr4);
    assert_eq!(vc.add(&vote5, 1), 1);
    assert_eq!(vc.get(&NilOrVal::Nil), 3);
    assert_eq!(vc.get(&NilOrVal::Val(val1)), 1);
    assert_eq!(vc.is_threshold_met(Threshold::Unreached, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Any, q, t), true);
    assert_eq!(vc.is_threshold_met(Threshold::Nil, q, t), true);
    assert_eq!(vc.is_threshold_met(Threshold::Value(val1), q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(val2), q, t), false);
}

#[test]
fn vote_count_value() {
    let t = 4;
    let q = ThresholdParam::TWO_F_PLUS_ONE;
    let h = Height::new(1);
    let r = Round::new(0);

    let mut vc = VoteCount::<TestContext>::new();

    let addr1 = Address::new([1; 20]);
    let addr2 = Address::new([2; 20]);
    let addr3 = Address::new([3; 20]);
    let addr4 = Address::new([4; 20]);

    let val1 = ValueId::new(1);
    let val2 = ValueId::new(2);
    let val3 = ValueId::new(3);

    assert_eq!(vc.get(&NilOrVal::Nil), 0);
    assert_eq!(vc.get(&NilOrVal::Val(val1)), 0);
    assert_eq!(vc.is_threshold_met(Threshold::Unreached, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Any, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Nil, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(val1), q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(val2), q, t), false);

    let vote1 = Vote::new_prevote(h, r, NilOrVal::Val(val1), addr1);
    assert_eq!(vc.add(&vote1, 1), 1);
    assert_eq!(vc.get(&NilOrVal::Nil), 0);
    assert_eq!(vc.get(&NilOrVal::Val(val1)), 1);
    assert_eq!(vc.is_threshold_met(Threshold::Unreached, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Any, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Nil, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(val1), q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(val2), q, t), false);

    let vote2 = Vote::new_prevote(h, r, NilOrVal::Val(val1), addr2);
    assert_eq!(vc.add(&vote2, 1), 2);
    assert_eq!(vc.get(&NilOrVal::Nil), 0);
    assert_eq!(vc.get(&NilOrVal::Val(val1)), 2);
    assert_eq!(vc.is_threshold_met(Threshold::Unreached, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Any, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Nil, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(val1), q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(val2), q, t), false);

    // addr1 votes again, for nil this time, is ignored
    let vote3 = Vote::new_prevote(h, r, NilOrVal::Nil, addr1);
    assert_eq!(vc.add(&vote3, 1), 0);
    assert_eq!(vc.get(&NilOrVal::Nil), 0);
    assert_eq!(vc.get(&NilOrVal::Val(val1)), 2);
    assert_eq!(vc.is_threshold_met(Threshold::Unreached, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Any, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Nil, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(val1), q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(val2), q, t), false);

    let vote4 = Vote::new_prevote(h, r, NilOrVal::Val(val1), addr3);
    assert_eq!(vc.add(&vote4, 1), 3);
    assert_eq!(vc.get(&NilOrVal::Nil), 0);
    assert_eq!(vc.get(&NilOrVal::Val(val1)), 3);
    assert_eq!(vc.is_threshold_met(Threshold::Unreached, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Any, q, t), true);
    assert_eq!(vc.is_threshold_met(Threshold::Nil, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(val1), q, t), true);
    assert_eq!(vc.is_threshold_met(Threshold::Value(val2), q, t), false);

    // addr2 votes again, for the same value, is ignored
    let vote5 = Vote::new_prevote(h, r, NilOrVal::Val(val1), addr2);
    assert_eq!(vc.add(&vote5, 1), 3);
    assert_eq!(vc.get(&NilOrVal::Nil), 0);
    assert_eq!(vc.get(&NilOrVal::Val(val1)), 3);
    assert_eq!(vc.is_threshold_met(Threshold::Unreached, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Any, q, t), true);
    assert_eq!(vc.is_threshold_met(Threshold::Nil, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(val1), q, t), true);
    assert_eq!(vc.is_threshold_met(Threshold::Value(val2), q, t), false);

    let vote6 = Vote::new_prevote(h, r, NilOrVal::Val(val2), addr4);
    assert_eq!(vc.add(&vote6, 1), 1);
    assert_eq!(vc.get(&NilOrVal::Nil), 0);
    assert_eq!(vc.get(&NilOrVal::Val(val1)), 3);
    assert_eq!(vc.get(&NilOrVal::Val(val2)), 1);
    assert_eq!(vc.is_threshold_met(Threshold::Unreached, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Any, q, t), true);
    assert_eq!(vc.is_threshold_met(Threshold::Nil, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(val1), q, t), true);
    assert_eq!(vc.is_threshold_met(Threshold::Value(val2), q, t), false);

    // addr4 votes again, for a different value, is ignored
    let vote7 = Vote::new_prevote(h, r, NilOrVal::Val(val3), addr4);
    assert_eq!(vc.add(&vote7, 1), 0);
    assert_eq!(vc.get(&NilOrVal::Nil), 0);
    assert_eq!(vc.get(&NilOrVal::Val(val1)), 3);
    assert_eq!(vc.get(&NilOrVal::Val(val2)), 1);
    assert_eq!(vc.get(&NilOrVal::Val(val3)), 0);
    assert_eq!(vc.is_threshold_met(Threshold::Unreached, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Any, q, t), true);
    assert_eq!(vc.is_threshold_met(Threshold::Nil, q, t), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(val1), q, t), true);
    assert_eq!(vc.is_threshold_met(Threshold::Value(val2), q, t), false);
}
