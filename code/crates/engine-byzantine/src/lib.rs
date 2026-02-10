//! Byzantine behavior support for the Malachite BFT consensus engine.
//!
//! This crate provides a [`ByzantineNetworkProxy`] actor that sits between the
//! consensus actor and the real network actor, intercepting outgoing messages to
//! simulate Byzantine faults such as equivocation, vote dropping, and more.
//!
//! It also provides a [`ByzantineMiddleware`] that can override vote construction
//! to simulate amnesia attacks (ignoring voting locks).
//!
//! # Configuration
//!
//! Byzantine behavior is configured via [`ByzantineConfig`], which is used to
//! configure the [`ByzantineNetworkProxy`] and [`ByzantineMiddleware`].

pub mod middleware;
pub mod proxy;
pub mod config;

pub use middleware::ByzantineMiddleware;
pub use proxy::ByzantineNetworkProxy;
pub use config::{ByzantineConfig, Trigger};
