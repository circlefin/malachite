use std::time::Duration;

use ractor::message::Message;
use ractor::ActorRef;

fn compute_adjusted_duration(base: Duration, adjustment: f64) -> Duration {
    let ref_nanos = base.as_nanos() as f64;
    let adj_nanos = (ref_nanos * (1.0 + adjustment)).round().max(1.0) as u128;
    Duration::from_nanos(adj_nanos.min(u64::MAX as u128) as u64)
}

pub async fn ticker<Msg>(
    ref_interval: Duration,
    target: ActorRef<Msg>,
    adjustment: f64,
    msg: impl Fn() -> Msg,
) where
    Msg: Message,
{
    let adj_interval = compute_adjusted_duration(ref_interval, adjustment);

    tracing::debug!(
        reference_interval = ?ref_interval,
        adjusted_interval = ?adj_interval,
        adjustment = format!("{:.2}%", adjustment * 100.0),
        "Initial ticker interval adjustment (provided factor)"
    );

    loop {
        tokio::time::sleep(adj_interval).await;

        if let Err(e) = target.cast(msg()) {
            tracing::error!(target = %target.get_id(), "Failed to send tick message: {e}");
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adjusted_interval_is_within_bounds_for_fractions() {
        let base = Duration::from_millis(1000);
        let fractions = [0.0, 0.01, 0.02, 0.04, 0.1, 0.2, 0.4, 0.5, 0.8, 1.0];
        for &max_fraction in &fractions {
            for &adjustment in &[
                -max_fraction,
                -max_fraction / 2.0,
                0.0,
                max_fraction / 2.0,
                max_fraction,
            ] {
                let adjusted = compute_adjusted_duration(base, adjustment);
                let base_ns = base.as_nanos() as i128;
                let adj_ns = adjusted.as_nanos() as i128;
                let lower = (base_ns as f64 * (1.0 - max_fraction)).floor() as i128;
                let upper = (base_ns as f64 * (1.0 + max_fraction)).ceil() as i128;
                assert!(lower <= adj_ns);
                assert!(adj_ns <= upper);
            }
        }
    }

    #[test]
    fn test_factor_based_adjustment_is_applied_and_clamped_min_1ns() {
        let base = Duration::from_nanos(10);
        // Large negative adjustment should still clamp to >= 1ns
        let adj = -0.99;
        let adj_interval = compute_adjusted_duration(base, adj);
        assert!(adj_interval.as_nanos() >= 1);

        // (Big) positive factor increases duration as expected
        let adj_pos = 1.5;
        let adj_pos_interval = compute_adjusted_duration(base, adj_pos);
        assert!(adj_pos_interval > base);
    }
}
