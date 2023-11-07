#![allow(clippy::bool_assert_comparison)]

use malachite_vote::count::VoteCount;
use malachite_vote::Threshold;

#[test]
fn vote_count_nil() {
    let mut vc = VoteCount::new(4, Default::default());

    let addr1 = [1];
    let addr2 = [2];
    let addr3 = [3];
    let addr4 = [4];

    assert_eq!(vc.get(&None), 0);
    assert_eq!(vc.get(&Some(1)), 0);
    assert_eq!(vc.is_threshold_met(Threshold::Unreached), false);
    assert_eq!(vc.is_threshold_met(Threshold::Any), false);
    assert_eq!(vc.is_threshold_met(Threshold::Nil), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(1)), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);

    assert_eq!(vc.add(addr1, None, 1), Threshold::Unreached);
    assert_eq!(vc.get(&None), 1);
    assert_eq!(vc.get(&Some(1)), 0);
    assert_eq!(vc.is_threshold_met(Threshold::Unreached), false);
    assert_eq!(vc.is_threshold_met(Threshold::Any), false);
    assert_eq!(vc.is_threshold_met(Threshold::Nil), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(1)), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);

    assert_eq!(vc.add(addr2, None, 1), Threshold::Unreached);
    assert_eq!(vc.get(&None), 2);
    assert_eq!(vc.get(&Some(1)), 0);
    assert_eq!(vc.is_threshold_met(Threshold::Unreached), false);
    assert_eq!(vc.is_threshold_met(Threshold::Any), false);
    assert_eq!(vc.is_threshold_met(Threshold::Nil), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(1)), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);

    // addr1 votes again, is ignored
    assert_eq!(vc.add(addr1, None, 1), Threshold::Unreached);
    assert_eq!(vc.get(&None), 2);
    assert_eq!(vc.get(&Some(1)), 0);
    assert_eq!(vc.is_threshold_met(Threshold::Unreached), false);
    assert_eq!(vc.is_threshold_met(Threshold::Any), false);
    assert_eq!(vc.is_threshold_met(Threshold::Nil), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(1)), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);

    assert_eq!(vc.add(addr3, None, 1), Threshold::Nil);
    assert_eq!(vc.get(&None), 3);
    assert_eq!(vc.get(&Some(1)), 0);
    assert_eq!(vc.is_threshold_met(Threshold::Unreached), false);
    assert_eq!(vc.is_threshold_met(Threshold::Any), true);
    assert_eq!(vc.is_threshold_met(Threshold::Nil), true);
    assert_eq!(vc.is_threshold_met(Threshold::Value(1)), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);

    assert_eq!(vc.add(addr4, Some(1), 1), Threshold::Any);
    assert_eq!(vc.get(&None), 3);
    assert_eq!(vc.get(&Some(1)), 1);
    assert_eq!(vc.is_threshold_met(Threshold::Unreached), false);
    assert_eq!(vc.is_threshold_met(Threshold::Any), true);
    assert_eq!(vc.is_threshold_met(Threshold::Nil), true);
    assert_eq!(vc.is_threshold_met(Threshold::Value(1)), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);
}

#[test]
fn vote_count_value() {
    let mut vc = VoteCount::new(4, Default::default());

    let addr1 = [1];
    let addr2 = [2];
    let addr3 = [3];
    let addr4 = [4];

    assert_eq!(vc.get(&None), 0);
    assert_eq!(vc.get(&Some(1)), 0);
    assert_eq!(vc.is_threshold_met(Threshold::Unreached), false);
    assert_eq!(vc.is_threshold_met(Threshold::Any), false);
    assert_eq!(vc.is_threshold_met(Threshold::Nil), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(1)), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);

    assert_eq!(vc.add(addr1, Some(1), 1), Threshold::Unreached);
    assert_eq!(vc.get(&None), 0);
    assert_eq!(vc.get(&Some(1)), 1);
    assert_eq!(vc.is_threshold_met(Threshold::Unreached), false);
    assert_eq!(vc.is_threshold_met(Threshold::Any), false);
    assert_eq!(vc.is_threshold_met(Threshold::Nil), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(1)), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);

    assert_eq!(vc.add(addr2, Some(1), 1), Threshold::Unreached);
    assert_eq!(vc.get(&None), 0);
    assert_eq!(vc.get(&Some(1)), 2);
    assert_eq!(vc.is_threshold_met(Threshold::Unreached), false);
    assert_eq!(vc.is_threshold_met(Threshold::Any), false);
    assert_eq!(vc.is_threshold_met(Threshold::Nil), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(1)), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);

    // addr1 votes again, for nil this time, is ignored
    assert_eq!(vc.add(addr1, None, 1), Threshold::Unreached);
    assert_eq!(vc.get(&None), 0);
    assert_eq!(vc.get(&Some(1)), 2);
    assert_eq!(vc.is_threshold_met(Threshold::Unreached), false);
    assert_eq!(vc.is_threshold_met(Threshold::Any), false);
    assert_eq!(vc.is_threshold_met(Threshold::Nil), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(1)), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);

    assert_eq!(vc.add(addr3, Some(1), 1), Threshold::Value(1));
    assert_eq!(vc.get(&None), 0);
    assert_eq!(vc.get(&Some(1)), 3);
    assert_eq!(vc.is_threshold_met(Threshold::Unreached), false);
    assert_eq!(vc.is_threshold_met(Threshold::Any), true);
    assert_eq!(vc.is_threshold_met(Threshold::Nil), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(1)), true);
    assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);

    // addr2 votes again, for the same value, is ignored
    assert_eq!(vc.add(addr2, Some(1), 1), Threshold::Value(1));
    assert_eq!(vc.get(&None), 0);
    assert_eq!(vc.get(&Some(1)), 3);
    assert_eq!(vc.is_threshold_met(Threshold::Unreached), false);
    assert_eq!(vc.is_threshold_met(Threshold::Any), true);
    assert_eq!(vc.is_threshold_met(Threshold::Nil), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(1)), true);
    assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);

    assert_eq!(vc.add(addr4, Some(2), 1), Threshold::Any);
    assert_eq!(vc.get(&None), 0);
    assert_eq!(vc.get(&Some(1)), 3);
    assert_eq!(vc.get(&Some(2)), 1);
    assert_eq!(vc.is_threshold_met(Threshold::Unreached), false);
    assert_eq!(vc.is_threshold_met(Threshold::Any), true);
    assert_eq!(vc.is_threshold_met(Threshold::Nil), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(1)), true);
    assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);

    // addr4 votes again, for a different value, is ignored
    assert_eq!(vc.add(addr4, Some(3), 1), Threshold::Any);
    assert_eq!(vc.get(&None), 0);
    assert_eq!(vc.get(&Some(1)), 3);
    assert_eq!(vc.get(&Some(2)), 1);
    assert_eq!(vc.get(&Some(3)), 0);
    assert_eq!(vc.is_threshold_met(Threshold::Unreached), false);
    assert_eq!(vc.is_threshold_met(Threshold::Any), true);
    assert_eq!(vc.is_threshold_met(Threshold::Nil), false);
    assert_eq!(vc.is_threshold_met(Threshold::Value(1)), true);
    assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);
}
