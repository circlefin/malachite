use crate::VotingPower;

/// Represents the different quorum thresholds.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Threshold<ValueId> {
    /// No quorum has been reached yet
    Unreached,

    /// Quorum of votes but not for the same value
    Any,

    /// Quorum of votes for nil
    Nil,

    /// Quorum (+2/3) of votes for a value
    Value(ValueId),
}

/// Represents the different quorum thresholds.
///
/// There are two thresholds:
/// - The quorum threshold, which is the minimum number of votes required for a quorum.
/// - The honest threshold, which is the minimum number of votes required for a quorum of honest nodes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ThresholdParams {
    /// Threshold for a quorum (default: 2f+1)
    pub quorum: ThresholdParam,

    /// Threshold for the minimum number of honest nodes (default: f+1)
    pub honest: ThresholdParam,
}

impl Default for ThresholdParams {
    fn default() -> Self {
        Self {
            quorum: ThresholdParam::TWO_F_PLUS_ONE,
            honest: ThresholdParam::F_PLUS_ONE,
        }
    }
}

/// Represents the different quorum thresholds.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ThresholdParam {
    /// Numerator of the threshold
    pub numerator: u64,

    /// Denominator of the threshold
    pub denominator: u64,
}

impl ThresholdParam {
    /// 2f+1, ie. more than two thirds of the total weight
    pub const TWO_F_PLUS_ONE: Self = Self::new(2, 3);

    /// f+1, ie. more than one third of the total weight
    pub const F_PLUS_ONE: Self = Self::new(1, 3);

    /// Create a new threshold parameter with the given numerator and denominator.
    pub const fn new(numerator: u64, denominator: u64) -> Self {
        Self {
            numerator,
            denominator,
        }
    }

    /// Check whether the threshold is met.
    pub fn is_met(&self, weight: VotingPower, total: VotingPower) -> bool {
        let lhs = weight
            .checked_mul(self.denominator)
            .expect("attempt to multiply with overflow");

        let rhs = total
            .checked_mul(self.numerator)
            .expect("attempt to multiply with overflow");

        lhs > rhs
    }

    /// Return the minimum expected weight to meet the threshold when applied to the given total.
    pub fn min_expected(&self, total: VotingPower) -> VotingPower {
        1 + total
            .checked_mul(self.numerator)
            .expect("attempt to multiply with overflow")
            .checked_div(self.denominator)
            .expect("attempt to divide with overflow")
    }
}

#[cfg(test)]
mod tests {
    use core::iter;
    use rand::{rngs::StdRng, Rng, SeedableRng};

    use super::*;

    #[test]
    fn threshold_param_is_met() {
        assert!(!ThresholdParam::TWO_F_PLUS_ONE.is_met(1, 3));
        assert!(!ThresholdParam::TWO_F_PLUS_ONE.is_met(2, 3));
        assert!(ThresholdParam::TWO_F_PLUS_ONE.is_met(3, 3));

        assert!(!ThresholdParam::F_PLUS_ONE.is_met(3, 10));
        assert!(ThresholdParam::F_PLUS_ONE.is_met(4, 10));
        assert!(!ThresholdParam::TWO_F_PLUS_ONE.is_met(6, 10));
        assert!(ThresholdParam::TWO_F_PLUS_ONE.is_met(7, 10));
    }

    #[test]
    #[should_panic(expected = "attempt to multiply with overflow")]
    fn threshold_param_is_met_overflow() {
        assert!(!ThresholdParam::TWO_F_PLUS_ONE.is_met(1, u64::MAX));
    }

    #[test]
    fn threshold_params_corner_cases() {
        let mut rng = StdRng::seed_from_u64(123456789);
        let max_total_power: u64 = 1u64 << 20; // ~10^6
        let mut total_power: u64 = 0;

        let steps = iter::from_fn(|| {
            let step = rng.gen_range(1..=20);
            total_power += step;
            Some(total_power)
        });

        for total in steps.take_while(|&v| v <= max_total_power) {
            let one_third_expected = ThresholdParam::F_PLUS_ONE.min_expected(total);
            let two_thirds_expected = ThresholdParam::TWO_F_PLUS_ONE.min_expected(total);
            // Assumption: f < n/3, take a margin before and after
            let power_margin = 3;
            let min_power = core::cmp::max(total / 3, power_margin + 1) - power_margin;
            let max_power = core::cmp::min(total / 3 + power_margin, total);
            for power in min_power..max_power {
                // Assumption: a quorum Q has more than 1/3 of the voting power
                let one_third = ThresholdParam::F_PLUS_ONE.is_met(power, total);
                assert!(
                    one_third == (3 * power > total),
                    "power = {power}, 3*power = {}, total = {total}, {one_third}",
                    3 * power,
                );
                assert!(
                    one_third == (power >= one_third_expected),
                    "power = {power}, total = {total}, one_third_expected = {one_third_expected}"
                );

                // Assumption: a quorum Q has twice more voting power than the remaining
                // Q = total - power; if Q is even, Q/2 > power; else (Q+1)/2 > power
                let two_thirds = ThresholdParam::TWO_F_PLUS_ONE.is_met(total - power, total);
                assert!(
                    two_thirds == ((total - power).div_ceil(2) > power),
                    "power = {power}, total - power = {}, total = {total}, {two_thirds}",
                    total - power,
                );
                assert!(
                    two_thirds == (total - power >= two_thirds_expected),
                    "power = {}, total = {total}, two_thirds_expected = {two_thirds_expected}",
                    total - power,
                );
            }
        }
    }
}
