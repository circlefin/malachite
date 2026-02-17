use std::collections::HashMap;
use std::time::{Duration, Instant};

use libp2p::PeerId;

/// Time window for rate limiting peers requests
const DEFAULT_RATE_WINDOW: Duration = Duration::from_secs(60);

/// Maximum peers requests allowed per peer within the rate window.
/// Set to 6 to allow initial request + 5 retries (Fibonacci backoff completes in ~12s)
const DEFAULT_MAX_REQUESTS_PER_WINDOW: u32 = 6;

/// Maximum violations before signaling disconnect.
/// After this many rate limit violations, the peer should be disconnected with backoff.
const DEFAULT_MAX_VIOLATIONS: u32 = 3;

/// Duration after which violations expire and are reset.
/// TODO: Remove this once peer banning system is implemented. The ban system will
/// call `clear_peer` when bans expire, making this expiry unnecessary.
const DEFAULT_VIOLATION_EXPIRY: Duration = Duration::from_secs(10 * 60); // 10 minutes

/// Result of a rate limit check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateLimitResult {
    /// Request is allowed
    Allowed,
    /// Request is rate limited but peer can stay connected
    RateLimited,
    /// Request is rate limited and peer should be disconnected (exceeded max violations)
    MaxViolations,
}

impl RateLimitResult {
    /// Returns true if the request is allowed.
    pub fn is_allowed(&self) -> bool {
        matches!(self, RateLimitResult::Allowed)
    }

    /// Returns true if the peer should be disconnected.
    pub fn should_disconnect(&self) -> bool {
        matches!(self, RateLimitResult::MaxViolations)
    }
}

/// Rate limiter for discovery peers requests.
///
/// Uses a fixed window approach that allows burst requests (to accommodate
/// retries with Fibonacci backoff) while still protecting against abuse.
///
/// Each peer gets a window that tracks:
/// - When the window started
/// - How many requests have been made in this window
///
/// Once the window expires, it resets for the next request.
///
/// Additionally tracks violations (rate limit hits) to support integration
/// with a peer reputation/banning system. After `max_violations` rate limit
/// hits, signals that the peer should be disconnected.
#[derive(Debug)]
pub struct DiscoveryRateLimiter {
    /// Tracks (window_start, request_count) per peer
    requests: HashMap<PeerId, (Instant, u32)>,
    /// Tracks (violation_count, last_violation_time) per peer
    violations: HashMap<PeerId, (u32, Instant)>,
    /// Duration of the rate limiting window
    rate_window: Duration,
    /// Maximum requests allowed per window
    max_requests_per_window: u32,
    /// Maximum violations before signaling disconnect
    max_violations: u32,
    /// Duration after which violations expire
    /// TODO: Remove once peer banning is implemented
    violation_expiry: Duration,
}

impl Default for DiscoveryRateLimiter {
    fn default() -> Self {
        Self::new(
            DEFAULT_RATE_WINDOW,
            DEFAULT_MAX_REQUESTS_PER_WINDOW,
            DEFAULT_MAX_VIOLATIONS,
            DEFAULT_VIOLATION_EXPIRY,
        )
    }
}

impl DiscoveryRateLimiter {
    /// Create a new rate limiter with custom settings.
    pub fn new(
        rate_window: Duration,
        max_requests_per_window: u32,
        max_violations: u32,
        violation_expiry: Duration,
    ) -> Self {
        Self {
            requests: HashMap::new(),
            violations: HashMap::new(),
            rate_window,
            max_requests_per_window,
            max_violations,
            violation_expiry,
        }
    }

    /// Check if a request from the given peer should be served.
    ///
    /// Returns `RateLimitResult::Allowed` if the request should be served,
    /// `RateLimitResult::RateLimited` or `RateLimitResult::MaxViolations` if rate limited.
    ///
    /// When rate limited, the violation count is incremented. If the violation
    /// count reaches `max_violations`, returns `MaxViolations` to signal disconnect.
    pub fn check_request(&mut self, peer_id: &PeerId) -> RateLimitResult {
        let now = Instant::now();

        // Check if violations have expired and clear them if so
        // TODO: Remove this expiry logic once peer banning is implemented
        if let Some((_, last_violation)) = self.violations.get(peer_id) {
            if now.duration_since(*last_violation) >= self.violation_expiry {
                // Clear both violations and request window for a fresh start
                self.violations.remove(peer_id);
                self.requests.remove(peer_id);
            }
        }

        // If peer already has max violations, reject immediately without any new requests
        let current_violations = self
            .violations
            .get(peer_id)
            .map(|(count, _)| *count)
            .unwrap_or(0);
        if current_violations >= self.max_violations {
            return RateLimitResult::MaxViolations;
        }

        if let Some((window_start, count)) = self.requests.get_mut(peer_id) {
            if now.duration_since(*window_start) < self.rate_window {
                // Within the same window
                if *count >= self.max_requests_per_window {
                    // Rate limited - increment violation count and update timestamp
                    let (violation_count, last_violation) =
                        self.violations.entry(*peer_id).or_insert((0, now));
                    *violation_count += 1;
                    *last_violation = now;

                    return if *violation_count >= self.max_violations {
                        RateLimitResult::MaxViolations
                    } else {
                        RateLimitResult::RateLimited
                    };
                }
                *count += 1;
            } else {
                // Window expired, start new window
                *window_start = now;
                *count = 1;
            }
        } else {
            // First request from this peer
            self.requests.insert(*peer_id, (now, 1));
        }

        RateLimitResult::Allowed
    }

    /// Get the current request count for a peer within the current window.
    /// Returns 0 if no requests have been made or if the window has expired.
    #[cfg(test)]
    pub fn request_count(&self, peer_id: &PeerId) -> u32 {
        let now = Instant::now();
        self.requests
            .get(peer_id)
            .filter(|(window_start, _)| now.duration_since(*window_start) < self.rate_window)
            .map(|(_, count)| *count)
            .unwrap_or(0)
    }

    /// Get the current violation count for a peer.
    /// Returns 0 if no violations have been recorded or if violations have expired.
    pub fn violation_count(&self, peer_id: &PeerId) -> u32 {
        let now = Instant::now();
        self.violations
            .get(peer_id)
            .filter(|(_, last_violation)| {
                now.duration_since(*last_violation) < self.violation_expiry
            })
            .map(|(count, _)| *count)
            .unwrap_or(0)
    }

    /// Remove rate limiting state for a peer (e.g., on disconnect).
    /// Note: This does NOT clear violation count, which persists across sessions
    /// to support the backoff/banning system.
    pub fn remove_peer(&mut self, peer_id: &PeerId) {
        self.requests.remove(peer_id);
        // Violations are intentionally NOT cleared - they persist for backoff/ban decisions
    }

    /// Clear all state for a peer, including violations.
    /// Use this when a peer's reputation has been restored or ban period has ended.
    #[cfg(test)]
    pub fn clear_peer(&mut self, peer_id: &PeerId) {
        self.requests.remove(peer_id);
        self.violations.remove(peer_id);
    }

    /// Get the rate window duration.
    pub fn rate_window(&self) -> Duration {
        self.rate_window
    }

    /// Get the maximum requests per window.
    pub fn max_requests_per_window(&self) -> u32 {
        self.max_requests_per_window
    }

    /// Get the maximum violations before disconnect.
    pub fn max_violations(&self) -> u32 {
        self.max_violations
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Use long expiry for tests so violations don't expire during tests
    const TEST_VIOLATION_EXPIRY: Duration = Duration::from_secs(3600);

    #[test]
    fn test_allows_requests_within_limit() {
        let mut limiter =
            DiscoveryRateLimiter::new(Duration::from_secs(60), 3, 3, TEST_VIOLATION_EXPIRY);
        let peer = PeerId::random();

        assert!(limiter.check_request(&peer).is_allowed());
        assert!(limiter.check_request(&peer).is_allowed());
        assert!(limiter.check_request(&peer).is_allowed());
        assert!(!limiter.check_request(&peer).is_allowed()); // 4th request blocked
    }

    #[test]
    fn test_request_count() {
        let mut limiter =
            DiscoveryRateLimiter::new(Duration::from_secs(60), 6, 3, TEST_VIOLATION_EXPIRY);
        let peer = PeerId::random();

        assert_eq!(limiter.request_count(&peer), 0);

        limiter.check_request(&peer);
        assert_eq!(limiter.request_count(&peer), 1);

        limiter.check_request(&peer);
        assert_eq!(limiter.request_count(&peer), 2);
    }

    #[test]
    fn test_remove_peer() {
        let mut limiter =
            DiscoveryRateLimiter::new(Duration::from_secs(60), 3, 3, TEST_VIOLATION_EXPIRY);
        let peer = PeerId::random();

        limiter.check_request(&peer);
        assert_eq!(limiter.request_count(&peer), 1);

        limiter.remove_peer(&peer);
        assert_eq!(limiter.request_count(&peer), 0);

        // Can make requests again after removal
        assert!(limiter.check_request(&peer).is_allowed());
    }

    #[test]
    fn test_independent_peers() {
        let mut limiter =
            DiscoveryRateLimiter::new(Duration::from_secs(60), 2, 3, TEST_VIOLATION_EXPIRY);
        let peer1 = PeerId::random();
        let peer2 = PeerId::random();

        assert!(limiter.check_request(&peer1).is_allowed());
        assert!(limiter.check_request(&peer1).is_allowed());
        assert!(!limiter.check_request(&peer1).is_allowed()); // peer1 blocked

        // peer2 should still be allowed
        assert!(limiter.check_request(&peer2).is_allowed());
        assert!(limiter.check_request(&peer2).is_allowed());
        assert!(!limiter.check_request(&peer2).is_allowed()); // peer2 blocked
    }

    #[test]
    fn test_violation_counting() {
        let mut limiter =
            DiscoveryRateLimiter::new(Duration::from_secs(60), 2, 3, TEST_VIOLATION_EXPIRY);
        let peer = PeerId::random();

        // Use up the quota
        assert!(limiter.check_request(&peer).is_allowed());
        assert!(limiter.check_request(&peer).is_allowed());

        // Now violations start
        assert_eq!(limiter.violation_count(&peer), 0);

        let result = limiter.check_request(&peer);
        assert!(!result.is_allowed());
        assert!(!result.should_disconnect()); // 1 violation, need 3
        assert_eq!(limiter.violation_count(&peer), 1);

        let result = limiter.check_request(&peer);
        assert!(!result.is_allowed());
        assert!(!result.should_disconnect()); // 2 violations
        assert_eq!(limiter.violation_count(&peer), 2);

        let result = limiter.check_request(&peer);
        assert!(!result.is_allowed());
        assert!(result.should_disconnect()); // 3 violations - disconnect!
        assert_eq!(limiter.violation_count(&peer), 3);
    }

    #[test]
    fn test_violations_persist_after_remove_peer() {
        let mut limiter =
            DiscoveryRateLimiter::new(Duration::from_secs(60), 1, 3, TEST_VIOLATION_EXPIRY);
        let peer = PeerId::random();

        // Use up quota and get a violation
        limiter.check_request(&peer);
        limiter.check_request(&peer); // violation 1
        assert_eq!(limiter.violation_count(&peer), 1);

        // Remove peer (simulates disconnect)
        limiter.remove_peer(&peer);

        // Violations should persist
        assert_eq!(limiter.violation_count(&peer), 1);

        // Request count should be reset
        assert_eq!(limiter.request_count(&peer), 0);

        // Can make requests again, but violations accumulate
        limiter.check_request(&peer);
        limiter.check_request(&peer); // violation 2
        assert_eq!(limiter.violation_count(&peer), 2);
    }

    #[test]
    fn test_clear_peer_removes_violations() {
        let mut limiter =
            DiscoveryRateLimiter::new(Duration::from_secs(60), 1, 3, TEST_VIOLATION_EXPIRY);
        let peer = PeerId::random();

        // Accumulate violations
        limiter.check_request(&peer);
        limiter.check_request(&peer);
        limiter.check_request(&peer);
        assert_eq!(limiter.violation_count(&peer), 2);

        // Clear peer completely
        limiter.clear_peer(&peer);

        // Everything should be reset
        assert_eq!(limiter.violation_count(&peer), 0);
        assert_eq!(limiter.request_count(&peer), 0);
    }

    #[test]
    fn test_max_violations_blocks_all_requests_after_reconnect() {
        // max_requests = 2, max_violations = 2
        let mut limiter =
            DiscoveryRateLimiter::new(Duration::from_secs(60), 2, 2, TEST_VIOLATION_EXPIRY);
        let peer = PeerId::random();

        // Use up quota: requests 1-2 allowed
        assert!(limiter.check_request(&peer).is_allowed());
        assert!(limiter.check_request(&peer).is_allowed());

        // Requests 3-4 rate limited, accumulate violations
        let result = limiter.check_request(&peer);
        assert!(!result.is_allowed());
        assert_eq!(limiter.violation_count(&peer), 1);

        let result = limiter.check_request(&peer);
        assert!(!result.is_allowed());
        assert_eq!(limiter.violation_count(&peer), 2);
        assert!(result.should_disconnect()); // max violations reached

        // Simulate disconnect - clears request window but keeps violations
        limiter.remove_peer(&peer);
        assert_eq!(limiter.request_count(&peer), 0);
        assert_eq!(limiter.violation_count(&peer), 2);

        // On reconnect: immediately rejected, no free requests allowed
        let result = limiter.check_request(&peer);
        assert!(!result.is_allowed());
        assert!(result.should_disconnect());
        // Violation count unchanged (not incremented again)
        assert_eq!(limiter.violation_count(&peer), 2);
    }

    #[test]
    fn test_violations_expire_after_duration() {
        // Use very short expiry for testing (1ms)
        let mut limiter =
            DiscoveryRateLimiter::new(Duration::from_secs(60), 1, 2, Duration::from_millis(1));
        let peer = PeerId::random();

        // Accumulate max violations
        limiter.check_request(&peer); // allowed
        limiter.check_request(&peer); // violation 1
        limiter.check_request(&peer); // violation 2 - should_disconnect
        assert_eq!(limiter.violation_count(&peer), 2);

        // Should be blocked immediately
        let result = limiter.check_request(&peer);
        assert!(!result.is_allowed());
        assert!(result.should_disconnect());

        // Wait for expiry
        std::thread::sleep(Duration::from_millis(5));

        // Violations should have expired - peer gets fresh start
        assert_eq!(limiter.violation_count(&peer), 0);

        // Should be allowed again
        let result = limiter.check_request(&peer);
        assert!(result.is_allowed());
    }

    #[test]
    fn test_rate_limit_result_methods() {
        let allowed = RateLimitResult::Allowed;
        assert!(allowed.is_allowed());
        assert!(!allowed.should_disconnect());

        let rate_limited = RateLimitResult::RateLimited;
        assert!(!rate_limited.is_allowed());
        assert!(!rate_limited.should_disconnect());

        let max_violations = RateLimitResult::MaxViolations;
        assert!(!max_violations.is_allowed());
        assert!(max_violations.should_disconnect());
    }
}
