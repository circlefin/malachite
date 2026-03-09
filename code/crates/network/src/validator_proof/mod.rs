//! Validator Proof Protocol
//!
//! A one-way protocol for validators to prove their identity to peers
//! by sending a signed proof.
//!
//! ## Wire Format
//!
//! ```text
//! [length: unsigned-varint][proof_bytes]
//! ```
//!
//! Uses unsigned-varint length prefix, consistent with libp2p request-response
//! and identify protocols. This is a one-way message with no response.
//!
//! ## Sending Proof
//!
//! The proof is set once at startup and sent automatically on every new connection:
//!
//! ```text
//! Startup:
//!   └─► behaviour.set_proof(proof_bytes)  — once
//!
//! ConnectionEstablished event:
//!   └─► behaviour.send_proof(peer_id)
//!       - Checks: has proof_bytes? first connection (other_established == 0)?
//!       └─► protocol::send_proof() spawned as task
//!           └─► Opens stream, writes proof, closes
//! ```
//!
//! The proof is a static binding of (public_key, peer_id) and does not change
//! with validator set membership. Whether the receiver classifies us as a
//! validator depends on their own validator set.
//!
//! ### Sending Guards (in `validator_proof/behaviour.rs`)
//! - `proof_bytes` must be set (set once at startup)
//! - `other_established == 0` gates sending to first connection only (via libp2p)
//!
//! ## Receiving & Validation
//!
//! ```text
//! Stream received
//!   └─► protocol::recv_proof()
//!       └─► Event::ProofReceived ──► network/lib.rs
//!           └─► Event::ValidatorProofReceived ──► engine/network.rs
//!               └─► NetworkEvent::ValidatorProofReceived ──► engine/consensus.rs
//!                   └─► NetworkMsg::ValidatorProofVerified ──► back to network
//! ```
//!
//! Validations at each layer:
//!
//! ### 1. `validator_proof/behaviour.rs` (Network Layer - Stream)
//! - **Message size**: Max 1KB enforced by codec
//! - **Stream read failure**: behaviour emits `CloseConnection` → DISCONNECT
//!
//! ### 2. `network/lib.rs` (Network Layer - Event Handling)
//! - Forwards proof to engine (anti-spam already handled by behaviour)
//!
//! ### 3. `engine/network.rs` (Engine Layer - Decoding)
//! - **Decode**: Proof bytes must decode as valid `ValidatorProof` → logged and ignored if not
//!   (see "Decode failures" below)
//! - **PeerId match**: `proof.peer_id` must equal sender's peer_id → DISCONNECT if not
//!   (prevents forwarding someone else's proof)
//!
//! ### 4. `engine/consensus.rs` (Consensus Layer - Cryptographic)
//! - **Signature verification**: Proof signature must be valid for the public key → DISCONNECT if not
//!
//! ### 5. `network/state.rs` (Network Layer - State)
//! - **Store proof**: `consensus_public_key` stored for validator set matching
//! - **Validator set check**: If public key matches a validator, mark peer as validator
//!   (proof is stored regardless, for re-evaluation when validator set changes)
//!
//! ## Failure Handling
//!
//! **Send failures** (`ProofSendFailed`):
//! - Forwarded to swarm; retry allowed on next connection or trigger
//!
//! **Receive failures** (`ProofReceiveFailed`):
//! - Cannot read stream (framing error, oversized message, connection drop)
//!   → behaviour emits `CloseConnection` → DISCONNECT
//!
//! **Anti-spam** (behaviour level):
//! - Duplicate proof from same peer → behaviour emits `CloseConnection` → DISCONNECT
//! - Tracked via `proofs_received` set, cleared when last connection closes
//!
//! **Decode failures** (application codec):
//! - Proof bytes received but cannot be decoded (e.g., `CodecError::UnsupportedVersion`)
//!   → logged and ignored (peer stays connected, just not classified as validator)
//!
//! **Validation failures** (after successful decoding):
//! - PeerId mismatch → DISCONNECT
//! - Invalid signature → DISCONNECT
//! - `Invalid` result sent back to network layer for disconnect

mod behaviour;
mod codec;
mod protocol;
mod types;

pub use behaviour::{Behaviour, Error, Event};
pub use types::ProofVerificationResult;
