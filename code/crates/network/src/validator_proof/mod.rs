//! Validator Proof Protocol
//!
//! A one-way protocol for validators to prove their identity to peers
//! by sending a signed proof.
//!
//! ## Wire Format
//!
//! Carried over `request_response`, but one-way on the wire:
//!
//! ```text
//! Request:  [length: unsigned-varint][proof_bytes]
//! Response: <no bytes>
//! ```
//!
//! The empty response only lets the request/response handler complete and close
//! its stream; it is not a delivery acknowledgement. See the `codec` module.
//!
//! ## Sending Proof
//!
//! The proof is set once at startup and sent on the first connection to a peer:
//!
//! ```text
//! Startup:
//!   └─► behaviour.set_proof(proof_bytes)  — once
//!
//! ConnectionEstablished (other_established == 0):
//!   └─► inner.send_request(peer, proof_bytes)
//! ```
//!
//! `ProofSent` marks local send completion, not confirmed remote delivery. There
//! is no same-session retry; a genuinely new connection starts a fresh session
//! and sends the proof again.
//!
//! The proof is a static binding of (public_key, peer_id) and does not change
//! with validator set membership. Whether the receiver classifies us as a
//! validator depends on their own validator set.
//!
//! ## Receiving & Validation
//!
//! ```text
//! Request received
//!   └─► Event::ProofReceived ──► network/lib.rs
//!       └─► Event::ValidatorProofReceived ──► engine/network.rs
//!           └─► NetworkEvent::ValidatorProofReceived ──► engine/consensus.rs
//!               └─► NetworkMsg::ValidatorProofVerified ──► back to network
//! ```
//!
//! Validations at each layer:
//!
//! ### 1. `validator_proof/behaviour.rs` (Network Layer)
//! - **Message size**: Max 1KB enforced by codec
//! - **Inbound read failure**: behaviour emits `CloseConnection` → DISCONNECT
//! - **Anti-spam**: a second proof from a peer this session → `CloseConnection`
//!
//! ### 2. `network/lib.rs` (Network Layer - Event Handling)
//! - Forwards proof to engine (anti-spam already handled by behaviour)
//!
//! ### 3. `engine/network.rs` (Engine Layer - Decoding)
//! - **Decode**: Proof bytes must decode as valid `ValidatorProof` → logged and ignored if not
//! - **PeerId match**: `proof.peer_id` must equal sender's peer_id → DISCONNECT if not
//!
//! ### 4. `engine/consensus.rs` (Consensus Layer - Cryptographic)
//! - **Signature verification**: Proof signature must be valid for the public key → DISCONNECT if not
//!
//! ### 5. `network/state.rs` (Network Layer - State)
//! - **Store proof**: `consensus_public_key` stored for validator set matching
//! - **Validator set check**: If public key matches a validator, mark peer as validator
//!
//! ## Failure Handling
//!
//! **Send failures** (`ProofSendFailed`):
//! - Forwarded to swarm; a new connection sends the proof again
//!
//! **Malformed requests**:
//! - A framing error, oversized message, truncated payload, or read timeout is
//!   delivered in-band as `ProofRequest::Malformed` (the codec never errors),
//!   which the behaviour turns into `CloseConnection` → DISCONNECT
//!
//! **Anti-spam** (behaviour level):
//! - A second proof from a peer this session → `CloseConnection` → DISCONNECT
//! - Tracked via `proofs_received`, cleared when the last connection closes
//!
//! **Decode failures** (application codec):
//! - Proof bytes received but cannot be decoded → logged and ignored (peer stays connected)
//!
//! **Validation failures** (after successful decoding):
//! - PeerId mismatch → DISCONNECT
//! - Invalid signature → DISCONNECT

mod behaviour;
mod codec;
mod types;

pub use behaviour::{Behaviour, Error, Event};
pub use types::ProofVerificationResult;
