# ADR 006: Proof-of-Validator Protocol

## Changelog

* 2025-12-17: Initial version
* 2025-12-23: Updated terminology and structure
* 2026-01-22: Simplified protocol - store and re-evaluate approach, no retries or responses

## Context

The Proof-of-Validator (PoV) protocol enables validator nodes to cryptographically prove their validator status to peers in the network.

Validators are critical participants in consensus, they are the only nodes allowed to sign and broadcast consensus messages.
Knowing which peers are validators enables better decisions about connection management.

**Use Cases:**

1. **Debugging and Observability**: Classify peers as validators vs other nodes for operational visibility
2. **Connection Prioritization**: Prefer connections with validators to ensure consensus message delivery
3. **Mesh Formation Optimization**: prioritize validators when building gossipsub's mesh overlay

**Scope:**

This protocol operates as an independent module. It receives validator set updates from an external component (e.g., the application layer in Malachite's model) and maintains its own copy of the current validator set. The protocol is agnostic to when or how often these updates occur—it simply uses the latest validator set to evaluate validator proofs.

## Decision

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│              External Component (e.g., Application)         │
│                                                             │
│  - Maintains validator set                                  │
│  - Provides updates to PoV module                           │
│                                                             │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           │ Validator Set Update
                           │ (eventually, frequency unspecified)
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                 Proof-of-Validator Module                   │
│                                                             │
│  - Stores local copy of current validator set               │
│  - Creates and stores validator proofs                      │
│  - Stores one proof per connected peer                      │
│  - Updates `is_validator` flag on validator set updates       │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**Implementation Note:** In Malachite, we plan to implement this protocol as part of the network layer. The validator set is already broadcast to the network component.

### Terminology

| Term | Definition |
|------|------------|
| **NodeId** | Unique identifier for a node on the network. In our implementation, we use libp2p's [`PeerId`](https://docs.libp2p.io/concepts/fundamentals/peers/#peer-id). |
| **ConsensusPubKey** | Consensus public key that uniquely identifies a validator. Used directly as the validator identifier in the validator set. |
| **Validator Set** | Set of consensus public keys representing the current validators. |
| **Validator Proof** | Binding between a NodeId and a ConsensusPubKey signed by the validator's consensus private key |
| **Prover** | Node sending a validator proof |
| **Verifier** | Node receiving, validating and storing a validator proof |

### Data Structures

#### ValidatorProof

```
ValidatorProof {
    node_id:        NodeId
    consensus_pub_key: ConsensusPubKey
    signature:         Signature
}
```

The proof binds a NodeId to a consensus public key, signed by the corresponding consensus private key. 

**Note:** Implementations should version this message following existing conventions.

#### Sign Bytes

The signature is computed over:

```
sign_bytes = PREFIX || len(consensus_pub_key) || consensus_pub_key || len(node_id) || node_id
```

Where `||` denotes byte concatenation and `len()` is the length in bytes encoded as a fixed-size integer.

**Prefix**: The 3-byte ASCII string `"PoV"` (0x50 0x6F 0x56) is prepended to the message for:
- Preventing cross-protocol signature reuse
- Easy identification in debugging and hex dumps
- Self-documenting (stands for "Proof-of-Validator")

**Length prefixes**: Including explicit length prefixes ensures future-proofing against changes in consensus public key or NetworkID format/length. 
This is important since Malachite is generic over the consensus signing scheme, and different key types may produce different signature lengths.

The signature is created using the consensus private key corresponding to `consensus_pub_key`.

#### Node State

Each node maintains the following global state for the Proof-of-Validator protocol:

```
NodeState {
    current_validator_set: ValidatorSet           // updated on each validator set update
    local_proof:           Option<ValidatorProof> // created at startup if node has consensus keypair
}
```

#### Per-Peer State

Each node maintains state for every connected peer:

```
PeerInfo {
    stored_proof:      Option<ValidatorProof>  // signature-verified proof from this peer (if any)
    is_validator:      bool                     // true if stored_proof.consensus_pub_key is in current_validator_set
}
```

**Key design properties:**

- **One proof per peer**: Each node stores at most one proof per connected peer.
   This bounds storage to the maximum number of connected peers.
    When a peer is disconnected, the state associated to it is removed.
- **Two-step validation**: (1) Signature verification happens once when the proof is received—only valid proofs are stored. (2) Validator status is determined by checking if `consensus_pub_key` is in the current validator set.
- **Handles promotions/demotions**: A stored proof remains valid across validator set changes. When a peer is demoted, `is_validator` becomes false but the proof stays stored. When re-promoted, `is_validator` becomes true again without needing a new proof.
- **No retries needed**: Proofs are validated once and stored; the `is_validator` flag is updated on validator set changes.

### Protocol Messages

#### ValidatorProof Message

Sent by a prover to claim validator status. This is a one-way message with no response.

```
ValidatorProofMessage {
    proof: ValidatorProof
}
```

### Protocol Flow

#### Proof Creation

A node with a consensus keypair creates a proof on startup. The proof binds the node's NodeId to its consensus public key.

```
On node startup:
    IF consensus_keypair is not None:
        node_id ← local NodeId
        sign_bytes ← PREFIX || len(consensus_keypair.pub_key) || consensus_keypair.pub_key || len(node_id) || node_id
        signature ← sign(consensus_keypair.private_key, sign_bytes)
        
        local_proof ← ValidatorProof {
            node_id,
            consensus_pub_key: consensus_keypair.pub_key,
            signature
        }
```

#### Peer Connection

When a new peer connects, initialize its `PeerInfo`. 
If the node has a consensus keypair, send the proof to the new peer.

```
On peer P connected:
    PeerInfo[P] ← {
        stored_proof: None,
        is_validator: false
    }
    
    IF local_proof is not None:
        send ValidatorProofMessage { proof: local_proof } to P
```

Sending the proof regardless of current validator status simplifies the protocol—there's no need to track "becoming a validator" events. The receiver stores the proof and updates `is_validator` whenever the validator set changes.

#### Receiving a Proof

When a node receives a proof from a peer, it validates and stores it. **Only one proof per peer is accepted**—if a peer sends multiple proofs, or an invalid proof, the node should disconnect from that peer (anti-spam protection).

```
On receive ValidatorProofMessage from peer P:
    proof ← message.proof
    
    // Anti-spam: only accept one proof per peer
    IF PeerInfo[P].stored_proof is not None:
        disconnect from P
        RETURN
    
    // Verify NodeId matches sender
    IF proof.node_id ≠ P.node_id:
        disconnect from P
        RETURN
    
    // Verify signature
    sign_bytes ← SEPARATOR || len(proof.consensus_pub_key) || proof.consensus_pub_key || len(proof.node_id) || proof.node_id
    IF NOT verify_signature(sign_bytes, proof.signature, proof.consensus_pub_key):
        disconnect from P
        RETURN
    
    // Store the valid proof
    PeerInfo[P].stored_proof ← proof
    
    // Evaluate against current validator set
    IF proof.consensus_pub_key IN current_validator_set:
        PeerInfo[P].is_validator ← true
```

#### Validator Set Updates

On receiving a validator set update, the node checks all stored proofs against the new validator set and updates the `is_validator` flag.

```
On validator_set_update(new_validator_set):
    current_validator_set ← new_validator_set
    
    // Update is_validator flag for all stored proofs
    FOR each peer P where PeerInfo[P].stored_proof is not None:
        IF PeerInfo[P].stored_proof.consensus_pub_key IN current_validator_set:
            PeerInfo[P].is_validator ← true
        ELSE:
            PeerInfo[P].is_validator ← false
```

**Key insight:** This approach handles timing differences naturally. 
If a peer sends a proof before the receiver knows the updated validator set,
the proof is stored and `is_validator` is updated when the receiver learns the updated validator set.

#### Peer Disconnection

When a peer disconnects, its `PeerInfo` is removed entirely. If it reconnects, it must send a new proof.

```
On peer P disconnected:
    DELETE PeerInfo[P]
```

#### Validator Identity Changes

When a validator changes its consensus key, NodeId, or both, it must disconnect and reconnect. On disconnect, its PeerInfo is deleted, so it always appears as a fresh peer when reconnecting with a new proof.

The constraints differ by case:
- **Consensus key change**: Must disconnect first, anti-spam protection rejects a second proof from the same peer
- **NodeId change**: Must shut down the old node first, two nodes must never use the same consensus key

### State Diagram

State transitions for a peer's `PeerInfo`:

```
                            on_connect
                                │
                                ▼
┌────────────────────────────────────────────────────────────────┐
│ stored_proof = None                                            │
│ is_validator = false                                           │
└───────────────────────────────┬────────────────────────────────┘
                                │
                                │ receive valid proof
                                ▼
┌────────────────────────────────────────────────────────────────┐
│ stored_proof = proof                                           │◄──┐
│ is_validator = (consensus_pub_key IN current_validator_set)    │   │ on validator_set_update:
└───────────────────────────────┬────────────────────────────────┘───┘ update current_validator_set
                                │
                                │ on_disconnect
                                ▼
┌────────────────────────────────────────────────────────────────┐
│ PeerInfo deleted                                               │
└────────────────────────────────────────────────────────────────┘
```

**Key points:**
- Proofs are bound to the connection with a peer
- `is_validator` is updated upon every validator set update
- A peer whose public key leaves and re-enters the validator set is automatically re-promoted (no new proof needed)

### Security Properties

#### Trust Assumptions

1. Consensus keys are securely stored and not compromised
2. Network keys are securely stored and not compromised
3. Validator set updates received are authentic
4. Signature scheme is secure

#### Attacks Within Trust Assumptions

Attacks that can occur even when trust assumptions hold:

| Threat | Description | Mitigated | How |
|--------|-------------|-----------|-----|
| **Replay attack** | Attacker intercepts a proof and tries to use it | Yes | Proof is bound to NodeId; verifier checks that proof's NodeId matches sender's actual NodeId |
| **Proof spam** | Attacker sends many proofs to waste resources | Yes | Only one proof per peer accepted; additional proofs trigger disconnection |
| **Former validator** | Peer was a validator but has been removed | Yes | `is_validator` checked against **current** validator set on each update |
| **Multiple nodes per validator** | Byzantine validator runs many nodes with different NodeIds, all proving the same consensus key | No | Wastes connection slots but does not affect consensus correctness. Potential fix: limit the number of connections per consensus key |

Proofs do not need a nonce or timestamp—they can be static (created once at startup) without expiration.
They remain valid as long as the peer remains connected.

#### Attacks When Trust Assumptions Are Broken

This section discusses what can happen when trust assumptions do not hold.

**Consensus key theft:**

If an attacker steals a validator's consensus private key, they can create valid proofs for any NodeId.
However, consensus key theft also enables equivocation in consensus, which is a far more serious issue than PoV compromise. 
Consensus keys must be securely stored.

**Network key theft:**

Network key theft is more probable than consensus key theft because network keys are typically stored in less secure mediums.
If an attacker steals a validator's network private key, it can operate a replay attack:

1. Retrieve a proof produced by the legit validator (via eavesdropping)
2. Connect to a node using the legit's validator NodeId
3. Submit the retrieved and legit validator's proof
4. Pass all verification checks

The impact depends on which feature uses the validator information:
- *Metrics/observability*: Shows a peer as validator when it is not
- *Connection prioritization*: Allows attacker to take connection slots reserved for validators

Potential mitigations (not implemented):
- Include a timestamp/expiry in the proof
- Use challenge-response where verifier sends a random challenge

**Validator set updates compromised:**

If an attacker can inject false validator set updates, they can make non-validators appear as validators or hide real validators. This breaks the protocol's ability to correctly identify validators. We do not plan to mitigate this at the PoV level.

**Signature scheme broken:**

If the cryptographic signature scheme is broken, attackers can forge proofs for any identity. This is a catastrophic failure that affects all cryptographic protocols, not just PoV. Mitigation is outside the scope of this protocol.

### Validator Set Timing Differences

The latest known validator sets of different nodes may differ at any given moment due to the nature of distributed systems.
The protocol handles this gracefully through the store-and-re-evaluate approach.

**How the store-and-re-evaluate approach handles this:**

1. **Validator added**: A node with a stale validator set receives a proof from a new validator. The proof is stored with `is_validator = false`. When the node receives the updated validator set, it sets `is_validator = true`.

2. **Validator removed**: Nodes that receive the update set `is_validator = false` immediately. 
Nodes that haven't yet received the update will keep `is_validator = true` until they get the update.

**Advantage:** No extra messages are needed. The proof is sent once and stored; the receiver automatically recognizes the validator when its validator set is updated.

## Alternative Approaches

### 1. Request-Response with Retries (Previously Considered)

In this approach, the prover sends a proof and expects a response indicating success or failure. 
If the verifier responds with `NotInValidatorSet`, the prover retries immediately.
The rationale is that a non-malicious prover receives this message because the verifier has not yet updated its validator set.

**Why we rejected it:**

- **Complexity**: Requires response handling, retry logic, timeout handling, and complex state machines (`Pending`, `Verified`, `Failed` states)
- **More messages**: Each proof exchange requires two messages (request + response), plus additional retry messages
- **Spam vulnerability**: Malicious peers could spam proof requests, triggering responses and creating amplification
- **No clear benefit**: The retry mechanism doesn't provide meaningful value—storing and re-evaluating achieves the same result with less complexity

### 2. Proof with Consensus Address Only

The proof could contain only the consensus address (derived from the public key) and the signature, without including the public key itself. To verify the proof, we would wait until the validator appears in a validator set update, which provides the public key needed for signature verification.

**Why we rejected it:**

By including `consensus_pub_key` in the proof, we can verify the signature immediately and reject invalid proofs on arrival. Without it, we would need to store unverified proofs and track their verification state until the validator appears in a validator set update.

## Status

Accepted

## Consequences

### Positive

- **Simple protocol**: No retry logic, response handling, or complex state machines
- **Bounded storage**: Number of stored proofs is bounded by maximum number of peers
- **Spam-resistant**: At most one proof per peer, disconnection in case of duplicates
- **Tolerates validator set differences**: Nodes may temporarily have different views of the validator set as they progress through different heights; store-and-re-evaluate handles this gracefully
- **Automatic re-promotion**: Validators leaving and re-entering the set are handled without re-sending proofs

### Negative

- **No immediate feedback**: Prover doesn't know if verifier accepted the proof
- **Memory for non-validators**: Proofs from peers not yet in the validator set are still stored
- **Unmitigated threats**: Not all threats are mitigated (see Security Properties)

### Neutral

- **Consensus key change**: Requires reconnection for peers to receive the updated proof

## References

- [libp2p Identify Protocol](https://github.com/libp2p/specs/tree/master/identify)
- [Ed25519 Signature Scheme](https://ed25519.cr.yp.to/)
