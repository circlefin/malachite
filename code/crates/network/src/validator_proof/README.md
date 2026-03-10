# Validator Proof Protocol

This module implements a one-way protocol that allows validators to prove their identity to peers. When a validator successfully proves their identity, peers may upgrade their GossipSub score, giving priority to validator messages in mesh formation and message propagation. In the future, this may also be used for connection prioritization (e.g., preferring connections to validators when slots are limited).

See ADR-006 (adr-006-proof-of-validator.md) for the design rationale and protocol specification.

## Overview

When peers connect, they don't know if the other peer is a validator. The Identify protocol provides a peer's moniker and listen address, but validator status must be cryptographically proven.

Each validator holds a pre-signed proof containing their consensus public key and libp2p peer ID, signed with their consensus private key. Validators send this proof:
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

## Validator Proof Related State

The validator proof state is split between two locations:

**`validator_proof::Behaviour`** (`behaviour.rs`) — connection-scoped session state:

| Field | Type | Purpose |
|-------|------|---------|
| `proof_bytes` | `Option<Bytes>` | Our proof to send (set once at startup if the node has a consensus key) |
| `proofs_received` | `HashSet<PeerId>` | Peers we've received from (anti-spam, cleared when last connection closes) |
| `listening` | `bool` | Whether the listener task has been spawned |

Connection tracking uses libp2p's built-in `other_established` (on `ConnectionEstablished`)
and `remaining_established` (on `ConnectionClosed`) instead of maintaining a separate map.
Proof is sent only on first connection (`other_established == 0`); state is cleaned up when the
last connection closes (`remaining_established == 0`).

All session state is cleared when the last connection to a peer closes, allowing fresh
exchange on reconnect.

**`State`** (`state.rs`) — persistent peer classification state:

| Field | Type | Purpose |
|-------|------|---------|
| `PeerInfo::consensus_public_key` | `Option<Vec<u8>>` | Stored public key from a verified proof. Used to re-evaluate validator status on validator set changes without needing a new proof. |
| `PeerInfo::consensus_address` | `Option<String>` | Derived address (set if public key matches a validator in the set, cleared if not). Used for display/metrics. |
| `PeerInfo::peer_type` | `PeerType` | Updated to `Validator` when proof is verified and key is in set. Updated on every validator set change via `reclassify_peers()`. |
| `pending_verified_proofs` | `HashMap<PeerId, Vec<u8>>` | Buffer for proofs verified before Identify completes (proof and Identify arrive in either order). Applied when `update_peer()` creates the `PeerInfo`. |

The split is because the **behaviour** handles the protocol mechanics
(when to send, what we've seen, anti-spam), while the **network state** handles the
durable classification (has this peer's proof been verified? are they in the validator set? what's their score?).

### Channels

| Channel | Direction | Type | Purpose |
|---------|-----------|------|---------|
| `events_tx/rx` | Send/Listener tasks → Behaviour | `mpsc::unbounded` | Internal: proof protocol results to behaviour's `poll()` |
| `tx_event` | Network task → Engine | `mpsc::channel(32)` | Network events (including `ValidatorProofReceived`) |
| `tx_ctrl` | Engine → Network task | `mpsc::channel(32)` | Control messages (including `ValidatorProofVerified`) |

## Protocol Flow

### Sending Proof

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        ON CONNECTION ESTABLISHED                            │
└─────────────────────────────────────────────────────────────────────────────┘

  behaviour.rs
  ┌──────────────────────────────────────────────────────────────────────────┐
  │ on_connection_established()                                              │
  │   ├─ Check: other_established == 0? (first connection to peer)           │
  │   └─ send_proof()                                                        │
  │        ├─ Check: proof_bytes.is_some()?                                  │
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
│                         PROOF LIFECYCLE                                     │
└─────────────────────────────────────────────────────────────────────────────┘

  network/lib.rs (startup)
  ┌──────────────────────────────────────────────────────────────────────────┐
  │ behaviour.set_proof(proof_bytes)  — once at startup                      │
  │                                                                          │
  │ On every new connection (ConnectionEstablished):                          │
  │   └─ behaviour.send_proof(peer_id)                                       │
  │       └─ (dedup via other_established == 0 check)                        │
  └──────────────────────────────────────────────────────────────────────────┘

  The proof is a static binding of (public_key, peer_id) and does not change
  with validator set membership. Whether the receiver classifies the sender
  as a validator depends on the receiver's own validator set.
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
  │ poll() - process protocol events (called from swarm.select_next_some())  │
  │   └─ ProofReceiveFailed → ToSwarm::CloseConnection (DISCONNECT)          │
  │   └─ ProofSendFailed → forward to swarm (allow retry)                    │
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
  │   │    └─ If fail → log warning, ignore (NO DISCONNECT)                  │
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
| First connection (send) | behaviour.rs (`other_established == 0`) | Skip send |
| proof_bytes set (send) | behaviour.rs | Skip send |
| Message size (1KB max) | codec.rs | Close stream |
| Stream read failure | behaviour.rs | Disconnect |
| Anti-spam (duplicate) | behaviour.rs | Disconnect |
| Decode proof | engine/network.rs | Log + ignore |
| PeerId matches sender | engine/network.rs | Disconnect |
| Signature valid | engine/consensus.rs | Disconnect |

### Checks that Must Stay in Engine

- **Decode** (engine/network.rs): Engine has the codec. Failures are logged and ignored (peer stays connected).
- **PeerId match** (engine/network.rs): Requires decoded proof
- **Signature verification** (engine/consensus.rs): Needs signing provider

## State Management

Connection-session state in `behaviour.rs`:
- `proofs_received: HashSet<PeerId>` — track peers we've received from (anti-spam)

Cleared when the last connection to a peer closes (`remaining_established == 0`), allowing
fresh exchange on reconnect.

Connection counting uses libp2p's built-in counters (`other_established` and
`remaining_established`) rather than maintaining a separate map.

## Scenario Diagrams

### Scenario 1: Node with Validator Key Connects to Peer

```
    Node A (has consensus key)                  Node B (Full Node)
         |                                            |
         |-------- TCP Connect ---------------------->|
         |                                            |
         |  [A has proof (set at startup)]            |
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

### Scenario 2: Invalid Proof - Disconnect

```
    Node A                                      Node B (malicious)
         |                                            |
         |<------- Validator Proof (invalid) ---------|
         |                                            |
         |  [A receives proof,                        |
         |   verification fails (bad signature        |
         |   or peer_id mismatch)]                    |
         |                                            |
         |======== Disconnect ========================|
         |                                            |
```

### Scenario 3: Duplicate Proof - Anti-spam

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

### Scenario 4: Incompatible Codec Version - Graceful Ignore

```
    Node A (new version)                        Node B (old version)
         |                                            |
         |<------- Validator Proof (old codec) -------|
         |                                            |
         |  [A receives proof bytes,                  |
         |   codec.decode() fails with                |
         |   CodecError::UnsupportedVersion]          |
         |                                            |
         |  [A logs warning, ignores proof]           |
         |  [B stays connected as full_node]          |
         |                                            |
```

## Upgrade Strategy

This protocol replaces `agent_version`-based validator classification with cryptographic proofs.

### What changed

| | Old behavior (`main`) | New behavior (`validator-proof`) |
|---|---|---|
| `agent_version` content | `moniker=X,address=Y` | `moniker=X` (no address) |
| Validator classification | Match `address` from `agent_version` against validator set | Cryptographic proof via `/malachitebft-validator-proof/v1` |

### Mixed network impact

During a rolling upgrade, old and new nodes coexist. The following peer classification
mismatches occur:

| Scenario | Classification | Correct? |
|---|---|---|
| **New node → new validator** | Proof received → `validator` | Yes |
| **New node → old validator** | No proof, no address in `agent_version` → `full_node` | **No** (under-classified) |
| **Old node → new validator** | No address in `agent_version` → `full_node` | **No** (under-classified) |
| **Old node → old validator** | Address in `agent_version` → `validator` | Yes |

In a mixed network, validators running different versions will be classified as `full_node`
by peers on the other version. This affects:

- **GossipSub scoring** (if enabled): Misclassified validators receive a lower score, making
  them more likely to be pruned from the mesh
- **Metrics and observability**: `discovered_peers` metric shows incorrect `peer_type`

This does **not** affect:

- **Consensus safety or liveness**: Consensus messages are delivered via GossipSub topic
  subscriptions regardless of peer type classification. A lower score may delay message
  delivery but does not prevent it.
- **Sync**: Sync operates independently of peer type classification.

### Recommended upgrade procedure

1. **Upgrade all nodes** to the new version. During the upgrade window, expect degraded
   peer classification (validators seen as `full_node` across version boundaries).
2. Once all nodes are upgraded, the validator proof protocol takes effect and all validators
   are correctly classified.

Falling back to `agent_version`-based classification was considered but rejected as it provides
lower security guarantees. A malicious peer could claim any validator's address in `agent_version`
without cryptographic proof, which is the exact attack this protocol prevents.

## Version Compatibility

The validator proof protocol has multiple versioned layers. This section explains what
happens when nodes running different versions connect.

### Protocol Layers and Their Versions

| Layer | Component | Version indicator | Example |
|-------|-----------|------------------|---------|
| **Protocol name** | libp2p multistream-select | Protocol string in `StreamProtocol` | `/malachitebft-validator-proof/v1` |
| **Wire codec** | `codec.rs` (unsigned-varint framing) | Implicit (framing format) | Length-delimited bytes |
| **Application codec** | Engine-level `codec.decode()` | Application-defined (e.g., explicit version field) | Codec V1 vs V2 |
| **Proof structure** | `ValidatorProof` fields | Defined by the application codec | Key length, signature scheme |

### What Happens on Mismatch

**1. Different protocol names** (e.g., `/malachitebft-validator-proof/v1` vs `/v2`)

The libp2p multistream-select negotiation fails. The stream is never opened, so no proof
is exchanged. The peer stays connected but is not classified as `validator`. No errors are
logged at the validator proof level — the negotiation failure is handled entirely by libp2p.

**2. Stream read failure** (e.g., oversized message, connection drop, corrupted bytes)

The wire codec uses standard unsigned-varint length-delimited framing, which is a libp2p
convention and will not change independently of a protocol name bump. A read failure here
indicates misbehavior (e.g., sending >1KB), a transient network error, or a bug — not a
version mismatch. The peer is **disconnected**.

**3. Same protocol name, different application codec** (e.g., codec version bump, different serialization format)

The wire codec successfully reads the bytes, but `engine/network.rs` fails to decode them
(e.g., `CodecError::UnsupportedVersion`). The error is **logged and ignored** — the peer
stays connected but is not classified as a validator.

Note: Adding new protobuf fields is generally backwards compatible (unknown fields are
ignored). This case applies when the codec has an explicit version check or when the
serialization format itself changes.

**4. Invalid signature**

The proof decodes successfully, but signature verification fails in `engine/consensus.rs`.
The peer is **disconnected**.

This can be caused by a forged signature (malicious peer) or by a signing scheme mismatch
during a rolling upgrade (e.g., one node uses ed25519, the other secp256k1). At the
verification layer, these two cases are indistinguishable.

To avoid disconnections during signing scheme changes, either use a different **protocol
name** (e.g., `/malachitebft-validator-proof/v1` → `/v2`), which reduces the problem to
case 1 (stream never opened), or a different **application codec version**, which reduces
it to case 3 (decode fails, logged and ignored). Both keep the peer connected.

### When to Change What

| What changed | Required change | Why |
|---|---|---|
| **Signing scheme** (e.g., ed25519 → secp256k1) | Protocol name or codec version | Without it, signature verification fails → disconnect (indistinguishable from forgery). |
| **Proof serialization** (e.g., new codec version) | Codec version | Decode failure is logged and ignored (case 3). |
| **Adding fields not in sign_bytes** | None | Backwards compatible — old decoders ignore unknown fields, signature is unaffected. |
| **Adding fields included in sign_bytes** | Protocol name or codec version | Old nodes reconstruct different sign_bytes → signature verification fails → disconnect. Either change is sufficient (case 1 or case 3). |

**Protocol name vs codec version change:** Changing the protocol name means no proof
exchange in either direction during a rolling upgrade. Changing only the codec version
(keeping the protocol name) allows the new codec to read both old and new formats, giving
one-directional verification (new nodes can verify old validators) during the upgrade window.

### Summary of Failure Outcomes

| Mismatch type | Stream opened? | Proof decoded? | Result |
|---------------|---------------|---------------|--------|
| Protocol name | No | No | Peer stays connected, classified as `full_node` |
| Stream read failure | Yes | No (read error) | Disconnect (misbehavior or network error) |
| Application codec | Yes | No (decode error) | Log + ignore, peer stays connected |
| Invalid signature | Yes | Yes | Disconnect (forgery or signing scheme mismatch — use different protocol name or codec versions to avoid) |
| Duplicate proof | Yes | Yes | Disconnect (anti-spam) |
| PeerId mismatch | Yes | Yes | Disconnect (proof forgery) |

### Design Rationale

Application codec decode failures are treated as non-fatal because they commonly occur
during rolling upgrades when nodes run different software versions. Disconnecting peers for
version mismatches would cause unnecessary churn and could impact consensus liveness if
enough validators are affected. When the new codec is backwards compatible (can read the
old format), keeping the same protocol name allows new nodes to still verify old validators'
proofs during the upgrade window.

Stream read failures result in disconnection because the wire framing (unsigned-varint
length-delimited) is a stable libp2p convention that does not change independently of
a protocol name bump. A read failure indicates misbehavior, a network error, or a bug.

Signature verification failures result in disconnection because a failed signature is
indistinguishable from a forgery attempt. Signing scheme changes should be accompanied
by a protocol name or codec version change to avoid this (see "When to Change What").

## Implementation Summary

- The protocol is enabled when `config.enable_consensus = true`
- Sync-only nodes do not enable the protocol
- The proof is set once at startup and sent to every new peer on `ConnectionEstablished`
- The proof is a static binding; validator set membership is evaluated by the receiver
- When the validator set changes, all peers with stored proofs are re-evaluated (`reclassify_peers`).
  Peers whose public key is no longer in the set are demoted (peer type and GossipSub score updated).

