# Validator Proof Protocol

This module implements a one-way protocol that allows validators to prove their identity to peers. When a validator successfully proves their identity, peers may upgrade their GossipSub score, giving priority to validator messages in mesh formation and message propagation. In the future, this may also be used for connection prioritization (e.g., preferring connections to validators when slots are limited).

See ADR-006 (adr-006-proof-of-validator.md) for the design rationale and protocol specification.

## Overview

When peers connect, they don't know if the other peer is a validator. The Identify protocol provides a peer's moniker and listen address, but validator status must be cryptographically proven.

Each validator holds a pre-signed proof containing their consensus public key and libp2p peer ID, signed with their consensus key. Validators send this proof:
1. On connection establishment (to new peers)
2. When becoming a validator (to existing peers)

The receiving peer verifies the signature and, if valid, marks the peer as a verified validator.

## Wire Format

This is a **one-way message** with no response (per ADR-006).

### Transport Framing (implementation choice)

The network layer (`codec.rs`) uses `unsigned-varint` length-delimited framing:
```
[unsigned-varint length prefix][proof_bytes]
```

This is consistent with libp2p's request-response and identify protocols. The codec also enforces a 1KB max message size (proofs are ~150 bytes for ed25519: 32-byte public key + 38-byte peer_id + 64-byte signature + serialization overhead).

### Proof Structure (per ADR-006)

The `proof_bytes` content is application-specific (serialized by the application's codec). ADR-006 specifies the proof structure with internal length prefixes for each field to support variable-length keys across different signing schemes.

The core type is `ValidatorProof` in `core-types`:

```rust
pub struct ValidatorProof<Ctx: Context> {
    /// The validator's consensus public key (raw bytes)
    pub public_key: Vec<u8>,
    /// The libp2p peer ID bytes
    pub peer_id: Vec<u8>,
    /// Signature over (public_key, peer_id) using the validator's consensus key
    pub signature: Signature<Ctx>,
}
```

See `test/src/codec/` for example serialization implementations (JSON, Protobuf).

## Protocol Flow

### Sending Proof

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        ON CONNECTION ESTABLISHED                            │
└─────────────────────────────────────────────────────────────────────────────┘

  behaviour.rs
  ┌──────────────────────────────────────────────────────────────────────────┐
  │ on_connection_established()                                              │
  │   ├─ Check: first connection? (connections HashMap)                      │
  │   └─ send_proof()                                                        │
  │        ├─ Check: proof_bytes.is_some()?                                  │
  │        ├─ Check: peer in proofs_sent?                                    │
  │        └─ spawn protocol::send_proof task                                │
  └──────────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
  protocol.rs
  ┌──────────────────────────────────────────────────────────────────────────┐
  │ send_proof()                                                             │
  │   └─ open_stream → write_proof → close                                   │
  │   └─ Return: Event::ProofSent or Event::ProofSendFailed                  │
  └──────────────────────────────────────────────────────────────────────────┘


┌─────────────────────────────────────────────────────────────────────────────┐
│                         ON BECOMING VALIDATOR                               │
└─────────────────────────────────────────────────────────────────────────────┘

  network/lib.rs (UpdateValidatorSet handler)
  ┌──────────────────────────────────────────────────────────────────────────┐
  │ if is_validator:                                                         │
  │   ├─ behaviour.set_proof(proof_bytes)                                    │
  │   └─ for each peer: behaviour.send_proof(peer_id)                        │
  │       └─ (behaviour handles dedup via proofs_sent)                       │
  │ else if was_validator:                                                   │
  │   └─ behaviour.clear_proof()                                             │
  └──────────────────────────────────────────────────────────────────────────┘
```

### Receiving Proof

```
  protocol.rs
  ┌──────────────────────────────────────────────────────────────────────────┐
  │ recv_proof() - incoming stream                                           │
  │   └─ Check: message size (codec, 1KB max)                                │
  │   └─ Return: Event::ProofReceived or Event::ProofReceiveFailed           │
  └──────────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
  behaviour.rs
  ┌──────────────────────────────────────────────────────────────────────────┐
  │ poll() - process protocol events                                         │
  │   └─ ProofReceiveFailed → ToSwarm::CloseConnection (DISCONNECT)          │
  │   └─ ProofSendFailed → remove from proofs_sent (allow retry)             │
  │   └─ ProofReceived:                                                      │
  │        └─ Check: peer in proofs_received? (ANTI-SPAM)                    │
  │             └─ If yes → ToSwarm::CloseConnection (DISCONNECT)            │
  │        └─ Add peer to proofs_received                                    │
  │        └─ Forward event to swarm                                         │
  │   └─ ProofSent → forward to swarm                                        │
  └──────────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
  network/lib.rs
  ┌──────────────────────────────────────────────────────────────────────────┐
  │ handle_validator_proof_event()                                           │
  │   └─ Forward: Event::ValidatorProofReceived{peer_id, proof_bytes}        │
  └──────────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
  engine/network.rs
  ┌──────────────────────────────────────────────────────────────────────────┐
  │ Msg::NewEvent(Event::ValidatorProofReceived)                             │
  │   ├─ Check: decode success? (codec.decode)                               │
  │   │    └─ If fail → send Invalid result                                  │
  │   ├─ Check: proof.peer_id == sender peer_id?                             │
  │   │    └─ If mismatch → send Invalid result                              │
  │   └─ Forward: NetworkEvent::ValidatorProofReceived{peer_id, proof}       │
  └──────────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
  engine/consensus.rs
  ┌──────────────────────────────────────────────────────────────────────────┐
  │ NetworkEvent::ValidatorProofReceived                                     │
  │   ├─ Check: signature valid? (verify_validator_proof)                    │
  │   ├─ Check: public_key in validator_set? (logging only)                  │
  │   └─ Send: NetworkMsg::ValidatorProofVerified{result, public_key}        │
  └──────────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
  network/lib.rs
  ┌──────────────────────────────────────────────────────────────────────────┐
  │ CtrlMsg::ValidatorProofVerified                                          │
  │   ├─ Check: result.is_verified()?                                        │
  │   │    └─ If invalid → DISCONNECT                                        │
  │   └─ If valid → record_verified_proof()                                  │
  └──────────────────────────────────────────────────────────────────────────┘
```

## Validation Checks

| Check | Location | On Failure |
|-------|----------|------------|
| First connection (send) | behaviour.rs | Skip send |
| proof_bytes set (send) | behaviour.rs | Skip send |
| Already sent to peer | behaviour.rs | Skip send |
| Message size (1KB max) | codec.rs | Close stream |
| Stream read failure | behaviour.rs | Disconnect |
| Anti-spam (duplicate) | behaviour.rs | Disconnect |
| Decode proof | engine/network.rs | Disconnect |
| PeerId matches sender | engine/network.rs | Disconnect |
| Signature valid | engine/consensus.rs | Disconnect |

### Checks that Must Stay in Engine

- **Decode** (engine/network.rs): Engine has the codec
- **PeerId match** (engine/network.rs): Requires decoded proof
- **Signature verification** (engine/consensus.rs): Needs signing provider

## State Management

All connection-session state is in `behaviour.rs`:
- `connections: HashMap<PeerId, HashSet<ConnectionId>>` - track active connections
- `proofs_sent: HashSet<PeerId>` - track peers we've sent to (dedup outgoing)
- `proofs_received: HashSet<PeerId>` - track peers we've received from (anti-spam)

All cleared when last connection to peer closes, allowing fresh exchange on reconnect.

## Scenario Diagrams

### Scenario 1: Validator Connects to Peer

```
    Node A (Validator)                          Node B (Full Node)
         |                                            |
         |-------- TCP Connect ---------------------->|
         |                                            |
         |  [A is validator, has proof]               |
         |                                            |
         |-------- Validator Proof ------------------>|
         |  (one-way, no response)                    |
         |                                            |
         |                       [B receives proof,
         |                        decodes & verifies signature,
         |                        stores consensus_public_key,
         |                        sets consensus_address if in valset]
         |                                            |
         |                       [B.peer_type = Validator]
         |                       [B updates GossipSub score for A]
         |                                            |
```

### Scenario 2: Node Becomes Validator

```
    Node A (becomes Validator)                  Node B (connected peer)
         |                                            |
         |  [A and B already connected]               |
         |                                            |
    ~~~~ Validator Set Update: A is now validator ~~~~
         |                                            |
         |  [A receives UpdateValidatorSet,           |
         |   learns it's now a validator,             |
         |   sets proof in behaviour]                 |
         |                                            |
         |-------- Validator Proof ------------------>|
         |                                            |
         |                       [B verifies & stores]
         |                       [B.peer_type = Validator]
         |                                            |
```

### Scenario 3: Invalid Proof - Disconnect

```
    Node A                                      Node B (malicious)
         |                                            |
         |<------- Validator Proof (invalid) ---------|
         |                                            |
         |  [A receives proof,                        |
         |   verification fails (bad signature,       |
         |   peer_id mismatch, or decode error)]      |
         |                                            |
         |======== Disconnect ========================|
         |                                            |
```

### Scenario 4: Duplicate Proof - Anti-spam

```
    Node A                                      Node B
         |                                            |
         |<------- Validator Proof (valid) -----------|
         |                                            |
         |  [A verifies & stores]                     |
         |                                            |
         |<------- Validator Proof (duplicate) -------|
         |                                            |
         |  [A detects duplicate in behaviour,        |
         |   peer already in proofs_received]         |
         |                                            |
         |======== Disconnect (anti-spam) ============|
         |                                            |
```

## Implementation Notes

- The protocol is enabled when `config.enable_consensus = true`
- Sync-only nodes do not enable the protocol
- Proof is only sent when we have `proof_bytes` set (i.e., we're a validator)
- When leaving the validator set, `clear_proof()` is called to stop sending proofs to new connections
