//! # Gossipsub Peer Scoring Implementation
//!
//! ## Summary
//! This module enables application-specific peer scoring for Gossipsub to prioritize nodes based on their types, in mesh formation and maintenance.
//! The scoring mechanism increases the chances that persistent peers preferentially connect to each other for consensus messages, while
//! still allowing full nodes to participate in the network.
//!
//! Without peer scoring, gossipsub uses a random mesh formation strategy where all peers have equal priority. This creates problems in mixed networks:
//! - Validator isolation: Full nodes can fill meshes before persistent peers/ validators connect to each other
//! - Suboptimal topology: Critical consensus messages may route through full nodes unnecessarily, or mesh partitions (eager push falls back to INEED/IWANT pull)
//! - Network resilience: Persistent peers/ validators-to-validator direct connections improve fault tolerance
//!
//! ## Score Assignment
//!
//! Peers receive different application-specific scores based on their type:
//!
//! - Persistent peers: `PERSISTENT_PEER_SCORE = 100.0` (10,000 after weight)
//! - Full nodes: `FULL_NODE_SCORE = 4.0` (400 after weight)
//! - Unknown: `UNKNOWN_PEER_SCORE = 0.0` (before Identify completes)
//!
//! The final peer score is: `app_specific_score × app_specific_weight`
//!
//! Note: We currently only use application-specific scoring based on peer type.
//! Topic-specific behavioral scoring (penalties for invalid messages, mesh misbehavior, etc.)
//! is not enabled.
//! Enabling topic-specific scoring will be done in a future release. This requires passing message
//! validation results from the application (malachite engine) layer to the network layer.
//!
//! ## Score Updates
//!
//! - On connection: Peer receives `UNKNOWN_PEER_SCORE = 0.0` to allow initial mesh formation
//! - After Identify: Score upgraded based on peer type (currently persistent peer or full node)
//!
//! ## Opportunistic Grafting
//!
//! GossipSub periodically evaluates mesh composition and gradually improves it by grafting
//! high-scoring peers:
//!
//! - Interval: Every `opportunistic_graft_ticks` heartbeats (default: 3 seconds)
//! - Batch size: Graft up to `opportunistic_graft_peers` at once (default: 2)
//! - Trigger condition: When **mesh median score** < `opportunistic_graft_threshold`
//! - Selection: Grafts peers with scores **above the current median** that are not in the mesh
//!
//! ## Thresholds
//!
//! - opportunistic_graft_threshold (100,000): Set very high to maximize high value node density. Triggers
//!   grafting whenever full nodes are present in the mesh, continuously replacing them with higher scored peers
//!   until the mesh is entirely made up of higher scored peers (or no more are available).
//! - gossip_threshold (-500): All nodes are well above this threshold and receive gossip messages.
//! - publish_threshold (-1,000): All nodes are well above this threshold and can publish messages.
//! - graylist_threshold (-2,000): All nodes are well above this threshold.
//!
//! Full nodes remain functional (can publish and receive gossip) but are aggressively replaced in
//! the mesh by higher scored peers through continuous opportunistic grafting.

use libp2p::gossipsub;

use crate::PeerType;

/// Application-Specific Scores
///
/// Application-specific score for validators.
///
/// After multiplication by `APP_SPECIFIC_WEIGHT` (100.0), validators have a score of 20,000.
pub const VALIDATOR_SCORE: f64 = 200.0;

/// Application-specific score for persistent peers.
///
/// After multiplication by `APP_SPECIFIC_WEIGHT` (100.0), persistent peers have a score of 10,000.
pub const PERSISTENT_PEER_SCORE: f64 = 100.0;

/// Application-specific score for full nodes.
///
/// After multiplication by `APP_SPECIFIC_WEIGHT` (100.0), full nodes have a score of 400.
pub const FULL_NODE_SCORE: f64 = 4.0;

/// Default score for newly connected peers before Identify completes.
/// Set to 0.0 to allow initial mesh formation without blocking on peer type detection.
pub const UNKNOWN_PEER_SCORE: f64 = 0.0;

/// Scoring Parameters
///
/// Weight multiplier for application-specific scores.
///
/// This amplifies the difference between nodes based on their type,
/// ensuring clear prioritization in opportunistic grafting.
const APP_SPECIFIC_WEIGHT: f64 = 100.0;

/// Threshold for opportunistic grafting.
///
/// Opportunistic grafting triggers when the **median score** of all mesh peers falls below this
/// threshold. It then grafts high-scoring peers (above the median) to improve mesh quality.
///
/// Setting this to a very high value (100,000) ensures grafting attempts to replace ANY full nodes
/// with validators whenever possible.
const OPPORTUNISTIC_GRAFT_THRESHOLD: f64 = 100_000.0;

/// Number of heartbeat ticks between opportunistic grafting attempts.
///
/// With a 1-second heartbeat interval, this means grafting is attempted every 3 seconds.
/// This is more aggressive than the libp2p default (60 ticks/seconds), allowing faster
/// mesh optimization to prioritize high scored peers over low scored peers.
pub const OPPORTUNISTIC_GRAFT_TICKS: u64 = 3;

/// Maximum number of peers to graft during each opportunistic grafting cycle.
///
/// This limits mesh churn while still allowing gradual optimization. Combined with
/// `OPPORTUNISTIC_GRAFT_TICKS`, this allows up to 40 peer replacements per minute if needed.
pub const OPPORTUNISTIC_GRAFT_PEERS: usize = 2;

/// Returns the default application score for a newly connected peer.
///
/// This score is set immediately when a peer connects, before the Identify protocol completes.
/// It allows gossipsub to form an initial mesh without blocking on peer type detection.
/// The score is upgraded after Identify completes and the peer type is determined.
pub fn get_default_score() -> f64 {
    UNKNOWN_PEER_SCORE
}

/// Returns the application-specific score for a given peer type.
///
/// This score is set after the Identify protocol completes and the peer's type is determined.
pub fn get_peer_score(peer_type: PeerType) -> f64 {
    if peer_type.is_validator() {
        VALIDATOR_SCORE
    } else if peer_type.is_persistent() {
        PERSISTENT_PEER_SCORE
    } else {
        FULL_NODE_SCORE
    }
}

/// Constructs the peer score parameters for GossipSub.
///
/// Configures application-specific scoring with a weight multiplier to amplify score differences.
pub fn peer_score_params() -> gossipsub::PeerScoreParams {
    gossipsub::PeerScoreParams {
        app_specific_weight: APP_SPECIFIC_WEIGHT,
        ..Default::default()
    }
}

/// Constructs the peer score thresholds for GossipSub.
///
/// Sets thresholds for opportunistic grafting and behavioral cutoffs:
/// - `opportunistic_graft_threshold`: Triggers grafting when mesh median score falls below this value
/// - `gossip_threshold`: Peers below this don't receive gossip
/// - `publish_threshold`: Peers below this can't publish messages
/// - `graylist_threshold`: Peers below this are completely ignored
pub fn peer_score_thresholds() -> gossipsub::PeerScoreThresholds {
    gossipsub::PeerScoreThresholds {
        opportunistic_graft_threshold: OPPORTUNISTIC_GRAFT_THRESHOLD,
        gossip_threshold: -500.0,
        publish_threshold: -1000.0,
        graylist_threshold: -2000.0,
        ..Default::default()
    }
}
