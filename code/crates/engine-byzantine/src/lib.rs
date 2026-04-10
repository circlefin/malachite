//! Byzantine behavior support for the Malachite BFT consensus engine.
//!
//! This crate provides a [`ByzantineNetworkProxy`] actor that sits between the
//! engine's `Consensus` actor (which drives the core consensus state machine)
//! and the real network actor, intercepting outgoing messages to simulate
//! Byzantine faults such as equivocation, vote dropping, and more.
//!
//! It also provides a [`ByzantineMiddleware`] that overrides prevote
//! construction to simulate amnesia attacks (ignoring voting locks).
//! Amnesia cannot be implemented at the proxy level because the consensus
//! engine applies the lock *before* emitting the vote to the network. By
//! that point the vote is already nil. The middleware intercepts earlier,
//! at prevote construction time, and substitutes the proposed value.
//!
//! # Configuration
//!
//! [`ByzantineConfig`] configures [`ByzantineNetworkProxy`]. Test-app
//! integrations typically map `ByzantineConfig::ignore_locks` into
//! [`ByzantineMiddleware`] construction.

pub mod config;
pub mod middleware;
pub mod proxy;

pub use config::{ByzantineConfig, Trigger};
pub use middleware::ByzantineMiddleware;
pub use proxy::{ByzantineNetworkProxy, ConflictingValueFn, ConflictingVoteValueFn};
