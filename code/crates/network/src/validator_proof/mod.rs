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
//! Proofs are sent in two scenarios:
//!
//! ### 1. On Connection Established (automatic)
//! ```text
//! ConnectionEstablished event
//!   └─► behaviour.on_connection_established()
//!       └─► behaviour.send_proof(peer_id)
//!           - Checks: has proof_bytes? first connection? not already sent?
//!           └─► protocol::send_proof() spawned as task
//!               └─► Opens stream, writes proof, closes
//! ```
//!
//! ### 2. On Validator Set Update (to existing peers)
//! ```text
//! CtrlMsg::UpdateValidatorSet
//!   └─► network/lib.rs: if is_validator
//!       └─► behaviour.set_proof(proof_bytes)
//!       └─► for each peer: behaviour.send_proof(peer_id)
//!           └─ (behaviour handles dedup via proofs_sent)
//! ```
//!
//! ### Sending Guards (in `validator_proof/behaviour.rs`)
//! - `proof_bytes` must be set (only set when `is_validator == true`)
//! - `proofs_sent` tracks peers we've sent to (prevents duplicates)
//! - `connections` tracks first vs additional connections (send only on first)
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
//! - **Decode**: Proof bytes must decode as valid `ValidatorProof` → DISCONNECT if not
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
//! - behaviour removes peer from `proofs_sent` set
//! - Retry allowed on next connection or trigger
//!
//! **Receive failures** (`ProofReceiveFailed`):
//! - Cannot read/decode stream → behaviour emits `CloseConnection` → DISCONNECT
//!
//! **Anti-spam** (behaviour level):
//! - Duplicate proof from same peer → behaviour emits `CloseConnection` → DISCONNECT
//! - Tracked via `proofs_received` set, cleared when last connection closes
//!
//! **Validation failures** (after decoding):
//! - PeerId mismatch → DISCONNECT
//! - Invalid signature → DISCONNECT
//! - `Invalid` result sent back to network layer for disconnect

mod behaviour;
mod codec;
mod protocol;
mod types;

pub use behaviour::{Behaviour, Error, Event};
pub use types::VerificationResult;
