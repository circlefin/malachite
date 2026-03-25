# Security Policy

## Reporting a Vulnerability

Please do not file public issues on Github for security vulnerabilities. All security vulnerabilities should be reported to Circle privately, through Circle's [Bug Bounty Program](https://hackerone.com/circle-bbp). Please read through the program policy before submitting a report.

### Scope

The following crates are in scope for the bug bounty program:

| Crate | Description |
| :---- | :---- |
| `app` | Application-level consensus parameters and height management |
| `app-channel` | Actor system builder and spawn infrastructure |
| `codec` | Codec trait for serialization/deserialization of all network messages |
| `config` | Consensus configuration (timeout config, parameters) |
| `core-consensus` | Core consensus state machine, effects, and decision logic |
| `core-driver` | Driver for consensus rounds, proposal tracking, polka certificates |
| `core-state-machine` | Timeout scheduling and round state transitions |
| `core-types` | Shared types: `CommitCertificate`, `MisbehaviorEvidence`, `ValidatorProof`, etc. |
| `core-votekeeper` | Vote collection, duplicate filtering, quorum computation, equivocation evidence |
| `discovery` | Peer discovery: address poisoning prevention, signed peer records, rate limiting |
| `engine` | Actor orchestration, WAL integration, timeout handling, sync response queuing |
| `network` | libp2p networking: peer scoring, IP limits, connection limits, signed peer records, address spoofing prevention, validator proof protocol |
| `signing` | `SigningProvider` trait (async, fallible) used by all consensus signing |
| `signing-ecdsa` | ECDSA implementations for secp256k1, P-256, P-384 curves |
| `signing-ed25519` | Ed25519 signing implementation |
| `sync` | Sync protocol: range validation, batch size caps, EMA scoring, status updates |
| `wal` | Write-ahead log: truncation, corruption detection, replay |
