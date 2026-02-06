use core::fmt;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use rand::distributions::weighted::WeightedIndex;
use rand::distributions::Distribution;
use rand::Rng;
use tracing::debug;

use malachitebft_peer::PeerId;

pub mod credit;
pub mod ema;
pub mod metrics;

use metrics::Metrics;

/// Result of a sync request to a peer
#[derive(Copy, Clone, Debug)]
pub enum SyncResult {
    /// Successful response with given response time
    Success(Duration),

    /// Timeout response
    Timeout,

    /// Failed response
    Failure,
}

pub type Score = f64;

/// Strategy for scoring peers based on sync results
pub trait ScoringStrategy: Send + Sync {
    /// Per-peer state maintained by this strategy
    type State: Default + Clone + Send + Sync;

    /// Initial score for new peers.
    ///
    /// ## Important
    /// The initial score MUST be in the `0.0..=1.0` range.
    fn initial_score(&self) -> Score;

    /// Update the peer score based on previous score and sync result.
    /// The strategy has mutable access to per-peer state.
    ///
    /// ## Important
    /// The updated score must be in the `0.0..=1.0` range.
    fn update_score(
        &self,
        state: &mut Self::State,
        previous_score: Score,
        result: SyncResult,
    ) -> Score;
}

/// Scoring strategies
#[derive(Copy, Clone, Debug, Default)]
pub enum Strategy {
    /// Exponential moving average strategy
    #[default]
    Ema,
    /// Credit-based strategy
    Credit,
}

/// Per-peer state maintained by the scorer
#[derive(Clone)]
pub struct PeerState<S: ScoringStrategy> {
    pub score: Score,
    pub last_update: Instant,
    pub strategy_state: S::State,
}

impl<S: ScoringStrategy> PeerState<S> {
    pub fn new(score: Score) -> Self {
        Self {
            score,
            last_update: Instant::now(),
            strategy_state: S::State::default(),
        }
    }
}

impl<S> fmt::Debug for PeerState<S>
where
    S: ScoringStrategy,
    S::State: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Round score to 3 decimal places for readability
        fn round_score(score: Score) -> f64 {
            (score * 1000.0).round() / 1000.0
        }

        f.debug_struct("PeerState")
            .field("score", &round_score(self.score))
            .field("last_update", &self.last_update.elapsed())
            .field("strategy_state", &self.strategy_state)
            .finish()
    }
}

/// Tracks peer scores using a scoring strategy
pub struct PeerScorer<S: ScoringStrategy> {
    state: HashMap<PeerId, PeerState<S>>,
    strategy: S,
}

impl<S: ScoringStrategy> PeerScorer<S> {
    /// Create a new peer scorer with specified strategy
    pub fn new(strategy: S) -> Self {
        Self {
            state: HashMap::new(),
            strategy,
        }
    }

    /// Update a peer's score based on the result of a sync request, recording the result in metrics.
    /// Returns the new score.
    pub fn update_score_with_metrics(
        &mut self,
        peer_id: PeerId,
        result: SyncResult,
        metrics: &Metrics,
    ) -> Score {
        let new_score = self.update_score(peer_id, result);
        metrics.observe_score(peer_id, new_score);
        new_score
    }

    /// Update a peer's score based on the result of a sync request.
    /// Returns the new score.
    pub fn update_score(&mut self, peer_id: PeerId, result: SyncResult) -> Score {
        let peer_state = self
            .state
            .entry(peer_id)
            .or_insert_with(|| PeerState::new(self.strategy.initial_score()));

        let previous_score = peer_state.score;

        debug!("Updating score for peer {peer_id}");
        debug!("  Result = {result:?}");
        debug!("    Prev = {previous_score}");

        let new_score =
            self.strategy
                .update_score(&mut peer_state.strategy_state, previous_score, result);
        debug!("     New = {new_score}");

        peer_state.score = new_score;
        peer_state.last_update = Instant::now();

        new_score
    }

    /// Get the current score for a peer
    pub fn get_score(&self, peer_id: &PeerId) -> Score {
        self.state
            .get(peer_id)
            .map(|p| p.score)
            .unwrap_or_else(|| self.strategy.initial_score())
    }

    /// Get all peer states
    pub fn get_state(&self) -> &HashMap<PeerId, PeerState<S>> {
        &self.state
    }

    /// Select a peer using weighted probabilistic selection
    pub fn select_peer<R: Rng>(&self, peers: &[PeerId], rng: &mut R) -> Option<PeerId> {
        if peers.is_empty() {
            return None;
        }

        let scores = peers.iter().map(|id| self.get_score(id).max(0.0));

        // Sample from peers using a weighted distribution based on their scores
        let distr = WeightedIndex::new(scores).ok()?;
        let index = distr.sample(rng);

        assert!(index < peers.len(), "Index out of bounds");
        Some(peers[index])
    }

    /// Prune peers whose scores have not been updated for the specified duration,
    /// effectively resetting their score to the initial score.
    ///
    /// A peer might be inactive because they were not picked for a long time
    /// due to their score being very low. Resetting their score gives them a chance to participate again.
    ///
    /// Note that by resetting the score we can also reduce the score of a peer,
    /// if the peer had a high score but was inactive for a long time.
    pub fn reset_inactive_peers_scores(&mut self, inactive_threshold: Duration) {
        let now = Instant::now();

        self.state.retain(|_, peer_state| {
            now.duration_since(peer_state.last_update) < inactive_threshold
        });
    }
}

impl Default for PeerScorer<ema::ExponentialMovingAverage> {
    fn default() -> Self {
        Self::new(ema::ExponentialMovingAverage::default())
    }
}

/// Enum wrapper for runtime-configurable scoring strategies
pub enum ScorerVariant {
    Ema(PeerScorer<ema::ExponentialMovingAverage>),
    Credit(PeerScorer<credit::Credit>),
}

impl ScorerVariant {
    /// Update a peer's score based on the result of a sync request.
    /// Returns the new score.
    pub fn update_score(&mut self, peer_id: PeerId, result: SyncResult) -> Score {
        match self {
            ScorerVariant::Ema(scorer) => scorer.update_score(peer_id, result),
            ScorerVariant::Credit(scorer) => scorer.update_score(peer_id, result),
        }
    }

    /// Update a peer's score based on the result of a sync request, recording the result in metrics.
    /// Returns the new score.
    pub fn update_score_with_metrics(
        &mut self,
        peer_id: PeerId,
        result: SyncResult,
        metrics: &Metrics,
    ) -> Score {
        match self {
            ScorerVariant::Ema(scorer) => {
                scorer.update_score_with_metrics(peer_id, result, metrics)
            }
            ScorerVariant::Credit(scorer) => {
                scorer.update_score_with_metrics(peer_id, result, metrics)
            }
        }
    }

    /// Get the current score for a peer
    pub fn get_score(&self, peer_id: &PeerId) -> Score {
        match self {
            ScorerVariant::Ema(scorer) => scorer.get_score(peer_id),
            ScorerVariant::Credit(scorer) => scorer.get_score(peer_id),
        }
    }

    /// Select a peer using weighted probabilistic selection
    pub fn select_peer<R: Rng>(&self, peers: &[PeerId], rng: &mut R) -> Option<PeerId> {
        match self {
            ScorerVariant::Ema(scorer) => scorer.select_peer(peers, rng),
            ScorerVariant::Credit(scorer) => scorer.select_peer(peers, rng),
        }
    }

    /// Prune peers whose scores have not been updated for the specified duration,
    /// effectively resetting their score to the initial score.
    pub fn reset_inactive_peers_scores(&mut self, inactive_threshold: Duration) {
        match self {
            ScorerVariant::Ema(scorer) => scorer.reset_inactive_peers_scores(inactive_threshold),
            ScorerVariant::Credit(scorer) => scorer.reset_inactive_peers_scores(inactive_threshold),
        }
    }

    /// Get all peer scores as a map from peer ID to score value (for debug logging)
    pub fn get_scores(&self) -> HashMap<PeerId, Score> {
        match self {
            ScorerVariant::Ema(scorer) => scorer
                .get_state()
                .iter()
                .map(|(id, state)| (*id, state.score))
                .collect(),
            ScorerVariant::Credit(scorer) => scorer
                .get_state()
                .iter()
                .map(|(id, state)| (*id, state.score))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arbtest::arbtest;
    use rand::rngs::StdRng;
    use rand::SeedableRng;
    use std::collections::HashSet;
    use std::ops::RangeInclusive;

    use arbtest::arbitrary::{Result, Unstructured};

    fn arb_response_time(u: &mut Unstructured) -> Result<Duration> {
        u.int_in_range(10..=5000).map(Duration::from_millis)
    }

    fn arb_response_time_fast(u: &mut Unstructured, slow_threshold: Duration) -> Result<Duration> {
        let max = slow_threshold.as_millis() as u64 - 10;
        u.int_in_range(10..=max).map(Duration::from_millis)
    }

    fn arb_response_time_slow(u: &mut Unstructured, slow_threshold: Duration) -> Result<Duration> {
        let min = slow_threshold.as_millis() as u64;
        let max = slow_threshold.as_millis() as u64 * 5;
        u.int_in_range(min..=max).map(Duration::from_millis)
    }

    fn arb_sync_result(u: &mut Unstructured) -> Result<SyncResult> {
        let result_type = u.int_in_range(0..=2)?;

        Ok(match result_type {
            0 => SyncResult::Success(arb_response_time(u)?),
            1 => SyncResult::Timeout,
            2 => SyncResult::Failure,
            _ => unreachable!(),
        })
    }

    fn arb_sync_result_success_fast(
        u: &mut Unstructured,
        slow_threshold: Duration,
    ) -> Result<SyncResult> {
        Ok(SyncResult::Success(arb_response_time_fast(
            u,
            slow_threshold,
        )?))
    }

    fn arb_sync_result_failure(u: &mut Unstructured) -> Result<SyncResult> {
        let result_type = u.int_in_range(0..=1)?;
        Ok(match result_type {
            0 => SyncResult::Timeout,
            1 => SyncResult::Failure,
            _ => unreachable!(),
        })
    }

    fn arb_scorer_variant(u: &mut Unstructured) -> Result<(ScorerVariant, Duration)> {
        let slow_threshold = Duration::from_millis(u.int_in_range(1000..=5000)?);
        eprintln!("slow_threshold = {slow_threshold:?}");

        let scorer = if u.arbitrary()? {
            eprintln!("Testing with EMA strategy");
            let strategy = arb_ema_strategy(u, slow_threshold)?;
            ScorerVariant::Ema(PeerScorer::new(strategy))
        } else {
            eprintln!("Testing with Credit strategy");
            let strategy = arb_credit_strategy(u, slow_threshold)?;
            ScorerVariant::Credit(PeerScorer::new(strategy))
        };

        Ok((scorer, slow_threshold))
    }

    fn arb_ema_strategy(
        u: &mut Unstructured,
        slow_threshold: Duration,
    ) -> Result<ema::ExponentialMovingAverage> {
        let alpha_success = u.choose(&[0.20, 0.25, 0.30])?;
        let alpha_timeout = u.choose(&[0.10, 0.15, 0.20])?;
        let alpha_failure = u.choose(&[0.10, 0.15, 0.20])?;

        Ok(ema::ExponentialMovingAverage::new(
            *alpha_success,
            *alpha_timeout,
            *alpha_failure,
            slow_threshold,
        ))
    }

    fn arb_credit_strategy(
        u: &mut Unstructured,
        slow_threshold: Duration,
    ) -> Result<credit::Credit> {
        let credit_fast_success = u.choose(&[30, 40, 50])?;
        let credit_slow_success = u.choose(&[-5, -10, -15])?;
        let credit_failure = u.choose(&[-15, -20, -25])?;
        let credit_timeout = u.choose(&[-10, -15, -20])?;

        Ok(credit::Credit::new(
            slow_threshold,
            *credit_fast_success,
            *credit_slow_success,
            *credit_failure,
            *credit_timeout,
            -100,
            100,
        ))
    }

    fn arb_vec<T>(
        u: &mut Unstructured,
        f: impl Fn(&mut Unstructured) -> Result<T>,
        size: RangeInclusive<usize>,
    ) -> Result<Vec<T>> {
        let size = u.int_in_range(size)?;
        (0..size).map(|_| f(u)).collect::<Result<Vec<T>>>()
    }

    // Helper to get strategy-specific properties for assertions
    fn update_score_directly(
        scorer: &ScorerVariant,
        state: &mut StrategyState,
        previous_score: Score,
        result: SyncResult,
    ) -> Score {
        match (scorer, state) {
            (ScorerVariant::Ema(s), StrategyState::Ema(ref mut peer_state)) => {
                s.strategy.update_score(peer_state, previous_score, result)
            }
            (ScorerVariant::Credit(s), StrategyState::Credit(ref mut peer_state)) => {
                s.strategy.update_score(peer_state, previous_score, result)
            }
            _ => unreachable!("Mismatched scorer and state types"),
        }
    }

    fn initial_score(scorer: &ScorerVariant) -> Score {
        match scorer {
            ScorerVariant::Ema(s) => s.strategy.initial_score(),
            ScorerVariant::Credit(s) => s.strategy.initial_score(),
        }
    }

    // Helper enum for per-peer state in tests
    enum StrategyState {
        Ema(<ema::ExponentialMovingAverage as ScoringStrategy>::State),
        Credit(<credit::Credit as ScoringStrategy>::State),
    }

    impl StrategyState {
        fn for_scorer(scorer: &ScorerVariant) -> Self {
            match scorer {
                ScorerVariant::Ema(_) => StrategyState::Ema(
                    <ema::ExponentialMovingAverage as ScoringStrategy>::State::default(),
                ),
                ScorerVariant::Credit(s) => {
                    // Initialize credit with the strategy's initial credit value
                    StrategyState::Credit(
                        s.strategy.min_credit + (s.strategy.max_credit - s.strategy.min_credit) / 2,
                    )
                }
            }
        }
    }

    // Property: Scores are bounded between 0.0 and 1.0
    #[test]
    fn scores_are_bounded() {
        arbtest(|u| {
            let (mut scorer, slow_threshold) = arb_scorer_variant(u)?;
            let results = arb_vec(
                u,
                |u| arb_sync_result_success_fast(u, slow_threshold),
                10..=100,
            )?;

            let peer_id = PeerId::random();

            // Initial score should be bounded
            let initial_score = scorer.get_score(&peer_id);
            assert!((0.0..=1.0).contains(&initial_score));

            // All updated scores should remain bounded
            for result in results {
                scorer.update_score(peer_id, result);
                let score = scorer.get_score(&peer_id);
                assert!(
                    (0.0..=1.0).contains(&score),
                    "Score {score} is out of bounds after result {result:?}",
                );
            }

            Ok(())
        });

        arbtest(|u| {
            let (mut scorer, _) = arb_scorer_variant(u)?;
            let results = arb_vec(u, arb_sync_result_failure, 10..=100)?;

            let peer_id = PeerId::random();

            // Initial score should be bounded
            let initial_score = scorer.get_score(&peer_id);
            assert!((0.0..=1.0).contains(&initial_score));

            // All updated scores should remain bounded
            for result in results {
                scorer.update_score(peer_id, result);
                let score = scorer.get_score(&peer_id);
                assert!(
                    (0.0..=1.0).contains(&score),
                    "Score {score} is out of bounds after result {result:?}",
                );
            }

            Ok(())
        });
    }

    // Property: Fast responses should improve the score
    #[test]
    fn fast_responses_improve_score() {
        arbtest(|u| {
            let (scorer, slow_threshold) = arb_scorer_variant(u)?;
            let response_time = arb_response_time_fast(u, slow_threshold)?;

            let init_score = initial_score(&scorer);
            let mut state = StrategyState::for_scorer(&scorer);

            let update_score = update_score_directly(
                &scorer,
                &mut state,
                init_score,
                SyncResult::Success(response_time),
            );

            assert!(
                update_score > init_score,
                "Fast response decreased score: {init_score} -> {update_score}",
            );

            Ok(())
        });
    }

    // Property: Slow responses should decrease the score
    #[test]
    fn slow_responses_decrease_score() {
        arbtest(|u| {
            let (scorer, slow_threshold) = arb_scorer_variant(u)?;
            let response_time = arb_response_time_slow(u, slow_threshold)?;

            let init_score = initial_score(&scorer);
            let mut state = StrategyState::for_scorer(&scorer);

            let update_score = update_score_directly(
                &scorer,
                &mut state,
                init_score,
                SyncResult::Success(response_time),
            );

            assert!(
                update_score < init_score,
                "Slow response should decrease score: {init_score} -> {update_score}",
            );

            Ok(())
        });
    }

    // Property: Failures and timeouts should decrease scores
    #[test]
    fn failures_decrease_score() {
        arbtest(|u| {
            let (scorer, _) = arb_scorer_variant(u)?;
            let failure_type = u.choose(&[SyncResult::Timeout, SyncResult::Failure])?;

            let init_score = initial_score(&scorer);
            let mut state = StrategyState::for_scorer(&scorer);

            let update_score =
                update_score_directly(&scorer, &mut state, init_score, *failure_type);

            assert!(
                update_score < init_score,
                "Failure/timeout should decrease score: {init_score} -> {update_score} for {failure_type:?}",
            );

            Ok(())
        });
    }

    // Property: Peer selection should be deterministic with same RNG seed
    #[test]
    fn peer_selection_is_deterministic() {
        arbtest(|u| {
            let peer_count = u.int_in_range(2usize..=10)?;
            let seed = u.arbitrary()?;
            let results = arb_vec(u, arb_sync_result, 0..=50)?;

            let peers: Vec<_> = (0..peer_count).map(|_| PeerId::random()).collect();

            let mut scorer1 = PeerScorer::default();
            let mut scorer2 = PeerScorer::default();

            // Apply same updates to both scorers
            for (i, result) in results.into_iter().enumerate() {
                let peer_id = peers[i % peers.len()];
                scorer1.update_score(peer_id, result);
                scorer2.update_score(peer_id, result);
            }

            // Select peers with same RNG seed
            let mut rng1 = StdRng::seed_from_u64(seed);
            let mut rng2 = StdRng::seed_from_u64(seed);

            for _ in 0..10 {
                let selection1 = scorer1.select_peer(&peers, &mut rng1);
                let selection2 = scorer2.select_peer(&peers, &mut rng2);
                assert_eq!(selection1, selection2);
            }

            Ok(())
        });
    }

    // Property: All peers should be selectable (no peer gets zero probability)
    #[test]
    fn all_peers_selectable() {
        arbtest(|u| {
            let peer_count = u.int_in_range(2_usize..=6)?;
            let results = arb_vec(u, arb_sync_result, 0..=20)?;

            let peers: Vec<PeerId> = (0..peer_count).map(|_| PeerId::random()).collect();
            let mut scorer = PeerScorer::default();

            // Apply random updates
            for (i, result) in results.iter().enumerate() {
                let peer_id = peers[i % peers.len()];
                scorer.update_score(peer_id, *result);
            }

            // Collect selections over many iterations
            let mut rng = StdRng::seed_from_u64(42);
            let mut selected_peers = HashSet::new();

            for _ in 0..1000 {
                if let Some(selected) = scorer.select_peer(&peers, &mut rng) {
                    selected_peers.insert(selected);
                }
            }

            // All peers should be selected at least once (with high probability)
            // Allow for some statistical variation by requiring at least 80% of peers
            let selection_ratio = selected_peers.len() as f64 / peers.len() as f64;
            assert!(
                selection_ratio >= 0.8,
                "Only {}/{} peers were selected",
                selected_peers.len(),
                peers.len()
            );

            Ok(())
        });
    }

    // Property: Higher scoring peers should be selected more frequently
    #[test]
    fn higher_scores_selected_more_frequently() {
        arbtest(|u| {
            let good_results = arb_vec(
                u,
                |u| {
                    u.choose_iter([
                        SyncResult::Success(Duration::from_millis(50)),
                        SyncResult::Success(Duration::from_millis(100)),
                    ])
                },
                5..=15,
            )?;

            let bad_results = arb_vec(
                u,
                |u| u.choose_iter([SyncResult::Timeout, SyncResult::Failure]),
                5..=15,
            )?;

            let good_peer = PeerId::random();
            let bad_peer = PeerId::random();
            assert_ne!(good_peer, bad_peer, "Peers should be distinct");

            let peers = vec![good_peer, bad_peer];

            let mut scorer = PeerScorer::default();

            // Give good peer good results
            for result in good_results {
                scorer.update_score(good_peer, result);
            }

            // Give bad peer bad results
            for result in bad_results {
                scorer.update_score(bad_peer, result);
            }

            let good_score = scorer.get_score(&good_peer);
            let bad_score = scorer.get_score(&bad_peer);

            // Only test if there's a meaningful difference in scores
            if good_score > bad_score + 0.1 {
                let mut rng = StdRng::seed_from_u64(123);
                let mut good_selections = 0;
                let mut bad_selections = 0;

                for _ in 0..1000 {
                    match scorer.select_peer(&peers, &mut rng) {
                        Some(peer) if peer == good_peer => good_selections += 1,
                        Some(peer) if peer == bad_peer => bad_selections += 1,
                        _ => {}
                    }
                }

                assert!(
                    good_selections > bad_selections,
                    "Good peer (score: {good_score}) selected {good_selections} times, bad peer (score: {bad_score}) selected {bad_selections} times"
                );
            }

            Ok(())
        });
    }

    // Property: Score updates should be monotonic for sequences of same result type
    #[test]
    fn monotonic_score_updates() {
        arbtest(|u| {
            let (scorer, slow_threshold) = arb_scorer_variant(u)?;
            let result = arb_sync_result(u)?;
            let update_count = u.int_in_range(1_usize..=20)?;

            let mut current_score = initial_score(&scorer);
            let mut state = StrategyState::for_scorer(&scorer);
            let mut scores = vec![current_score];

            for _ in 0..update_count {
                current_score = update_score_directly(&scorer, &mut state, current_score, result);
                scores.push(current_score);
            }

            // Check monotonicity based on result type
            match result {
                SyncResult::Success(rt) if rt < slow_threshold => {
                    // For fast response, scores should increase
                    for window in scores.windows(2) {
                        let diff = window[1] - window[0];
                        assert!(
                            diff >= 0.0,
                            "Fast response should improve score: {} -> {}",
                            window[0],
                            window[1]
                        );
                    }
                }
                SyncResult::Success(_) => {
                    // For slow responses, scores should decrease
                    for window in scores.windows(2) {
                        assert!(
                            window[1] <= window[0],
                            "Slow response should decrease score: {} -> {}",
                            window[0],
                            window[1]
                        );
                    }
                }
                SyncResult::Timeout | SyncResult::Failure => {
                    // For failures, scores should decrease
                    for window in scores.windows(2) {
                        assert!(
                            window[1] <= window[0],
                            "Timeouts and failures should decrease score: {} -> {}",
                            window[0],
                            window[1]
                        );
                    }
                }
            }

            Ok(())
        });
    }

    // Property: Empty peer list should return None
    #[test]
    fn empty_peer_list_returns_none() {
        arbtest(|u| {
            let seed = u.arbitrary()?;

            let scorer = PeerScorer::default();
            let mut rng = StdRng::seed_from_u64(seed);
            let result = scorer.select_peer(&[], &mut rng);
            assert_eq!(result, None);
            Ok(())
        });
    }

    // Property: Single peer should always be selected
    #[test]
    fn single_peer_always_selected() {
        arbtest(|u| {
            let seed = u.arbitrary()?;
            let results = arb_vec(u, arb_sync_result, 0..=10)?;

            let peer = PeerId::random();
            let peers = vec![peer];
            let mut scorer = PeerScorer::default();

            // Apply some updates
            for result in results {
                scorer.update_score(peer, result);
            }

            let mut rng = StdRng::seed_from_u64(seed);
            for _ in 0..10 {
                let selected = scorer.select_peer(&peers, &mut rng);
                assert_eq!(selected, Some(peer));
            }

            Ok(())
        });
    }

    // Property: Response time affects success score quality
    #[test]
    fn response_time_affects_success_score() {
        arbtest(|u| {
            let (scorer, _) = arb_scorer_variant(u)?;
            let fast_time = u.int_in_range(10_u64..=100)?;
            let slow_time = u.int_in_range(1000_u64..=5000)?;

            let init_score = initial_score(&scorer);
            let mut fast_state = StrategyState::for_scorer(&scorer);
            let mut slow_state = StrategyState::for_scorer(&scorer);

            let fast_result = SyncResult::Success(Duration::from_millis(fast_time));
            let slow_result = SyncResult::Success(Duration::from_millis(slow_time));

            let fast_score =
                update_score_directly(&scorer, &mut fast_state, init_score, fast_result);
            let slow_score =
                update_score_directly(&scorer, &mut slow_state, init_score, slow_result);

            assert!(
                fast_score >= slow_score,
                "Fast response ({fast_time} ms) should score >= slow response ({slow_time} ms): {fast_score} vs {slow_score}"
            );

            Ok(())
        });
    }

    // Property: Updating a peer's score does not affect other peers' scores
    #[test]
    fn updating_one_peer_does_not_affect_others() {
        arbtest(|u| {
            let (mut scorer, _) = arb_scorer_variant(u)?;
            let results = arb_vec(u, arb_sync_result, 0..=10)?;

            let peer1 = PeerId::random();
            let peer2 = PeerId::random();

            // Update peer1 with some results
            for result in &results {
                scorer.update_score(peer1, *result);
            }

            // Get initial score for peer2
            let initial_score_peer2 = scorer.get_score(&peer2);

            // Update peer1 again
            for result in &results {
                scorer.update_score(peer1, *result);
            }

            // Score for peer2 should remain unchanged
            let final_score_peer2 = scorer.get_score(&peer2);
            assert_eq!(initial_score_peer2, final_score_peer2);

            Ok(())
        });
    }

    // Property: Fast responses help a peer recover more quickly than timeouts penalize it
    #[test]
    fn fast_response_help_recover_score_quickly() {
        arbtest(|u| {
            let (mut scorer, slow_threshold) = arb_scorer_variant(u)?;
            let response_time = arb_response_time_fast(u, slow_threshold)?;

            let peer_id = PeerId::random();

            let init_score = scorer.get_score(&peer_id);

            // Apply a timeout
            scorer.update_score(peer_id, SyncResult::Timeout);
            let score_after_timeout = scorer.get_score(&peer_id);

            // Score after success should be higher than after timeout
            assert!(
                score_after_timeout < init_score,
                "Score after timeout ({score_after_timeout}) should be lower than initial score ({init_score})"
            );

            // Apply a success
            scorer.update_score(peer_id, SyncResult::Success(response_time));
            let score_after_success = scorer.get_score(&peer_id);

            // Score after success should be higher than initial score
            assert!(
                score_after_success > init_score,
                "Score after success ({score_after_success}) should be greater than initial score ({init_score})"
            );

            Ok(())
        });
    }

    // Property: Pruning inactive peers resets their scores
    #[test]
    fn pruning_inactive_peers_resets_scores() {
        arbtest(|u| {
            let (mut scorer, _) = arb_scorer_variant(u)?;

            let peer_id = PeerId::random();
            let init_score = initial_score(&scorer);

            // Update score for the peer
            scorer.update_score(peer_id, SyncResult::Success(Duration::from_millis(100)));

            // Prune inactive peers with a threshold that will remove this peer
            scorer.reset_inactive_peers_scores(Duration::from_millis(0));

            // Peer should be removed, score should reset to initial
            assert_eq!(scorer.get_score(&peer_id), init_score);

            Ok(())
        });
    }
}
