//! Byzantine behavior support for the Malachite BFT consensus engine.
//!
//! This crate provides a [`ByzantineNetworkProxy`] actor that sits between the
//! engine's `Consensus` actor (which drives the core consensus state machine)
//! and the real network actor, intercepting outgoing messages to simulate
//! Byzantine faults such as equivocation, vote dropping, and more.
//!
//! It also provides a context-generic [`Amnesia`] tracker that
//! implements the amnesia state machine (ignoring voting locks). Amnesia
//! cannot be implemented at the proxy level because the consensus engine
//! applies the lock *before* emitting the vote to the network; by that point
//! the vote is already nil. `Amnesia` is designed to be embedded
//! into each integration's prevote-construction path.
//!
//! Receiver-side filtering (e.g. dropping selected inbound proposals) is
//! handled by [`InboundFilter`], a small actor that the proxy splices between
//! the real network's output port and the consensus subscriber when
//! `drop_inbound_proposals` is configured.
//!
//! The crate does NOT ship default [`ConflictingValueFn`] /
//! [`ConflictingVoteValueFn`] factories: the core `Value` trait has no
//! byte-access contract, so producing a "different" value from an original
//! is inherently a downstream concern. Integrations supply their own
//! closures (typically a one-liner flipping the last byte of a hash).
//!
//! # Configuration
//!
//! [`ByzantineConfig`] configures [`ByzantineNetworkProxy`]. Integrations
//! typically map `ByzantineConfig::ignore_locks` into [`Amnesia`]
//! construction. For the test app, the `Middleware` adapter lives in the
//! `malachitebft_test::byzantine::ByzantineMiddleware` type; downstream
//! contexts embed [`Amnesia`] directly in their own prevote path.

pub mod amnesia;
pub mod config;
pub mod inbound;
pub mod proxy;

pub use amnesia::Amnesia;
pub use config::{ByzantineConfig, Trigger};
pub use inbound::{InboundFilter, InboundFilterMsg};
pub use proxy::{ByzantineNetworkProxy, ConflictingValueFn, ConflictingVoteValueFn};
