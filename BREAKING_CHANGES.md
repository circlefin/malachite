# Breaking Changes

## Unreleased

## 0.8.0

*August 27th, 2026*

### `malachitebft-core-types`

- Added a `max_timeout` field to `LinearTimeouts` (defaults to 60s); `LinearTimeouts::duration_for` now clamps per-round timeouts (`Propose`, `Prevote`, `Precommit`, `Rebroadcast`) to this cap
- Removed the `ValuePayload::PartsOnly` variant and the `ValuePayload::parts_only()` helper. Applications that disseminated values without an explicit `Proposal` message must migrate to `ValuePayload::ProposalAndParts`.
- Added new `VoteExtensionScope<Ctx> { height, round, value_id, validator_address }` type used to bind vote-extension signatures to their precommit
- Added new `ExtendedCommitSignature<Ctx>` and `ExtendedCommitCertificate<Ctx>` types. `ExtendedCommitCertificate` bundles per-validator commit signatures together with their optional vote extensions in a single self-verifiable object. Construct via `ExtendedCommitCertificate::from_votes` (from precommit votes that may still carry extensions) or `ExtendedCommitCertificate::from_commit_certificate_and_extensions(certificate, extensions)` (rebuilding from the parallel `(CommitCertificate, VoteExtensions)` shape exposed by the host API). Project back down via `trim_vote_extensions()`.
- Added `CertificateError::InvalidVoteExtensionSignature(Ctx::Address)` variant.
- Added `CertificateError::VotingPowerOverflow { signed, added }` variant, returned when commit-certificate verification would overflow while accumulating signed voting power.
- Added `Hash` to the `Height` trait's supertrait bounds. Custom `Height` implementations must now also implement `Hash` (all built-in heights already do). Required so `Ctx::Height` can key the sync layer's retry-backoff timers.
- `ValueResponse<Ctx>.certificate` is now `ExtendedCommitCertificate<Ctx>` instead of `CommitCertificate<Ctx>`. The constructor signature changes accordingly.
- Added `VoteExtensionPolicy` and a `vote_extension_policy` field to `HeightParams<Ctx>`. `HeightParams::new` defaults it to `VoteExtensionPolicy::Disabled`, meaning non-nil precommits must not carry vote extensions; applications that require every non-nil precommit to carry a vote extension should call `.with_vote_extension_policy(VoteExtensionPolicy::Required)` for those heights.

### `malachitebft-signing`

- `Signer::sign_vote_extension` and `Verifier::verify_signed_vote_extension` now take a `VoteExtensionScope<Ctx>` argument so the signature binds the extension to `(height, round, value_id, validator_address)`. The wire format of `SignedExtension` is unchanged; only the signed preimage changes — implementations must rebuild the canonical envelope to keep producing/verifying compatible signatures.
  - Old: `sign_vote_extension(&self, extension: Ctx::Extension) -> Result<SignedExtension<Ctx>, Error>`
  - New: `sign_vote_extension(&self, scope: VoteExtensionScope<Ctx>, extension: Ctx::Extension) -> Result<SignedExtension<Ctx>, Error>`
  - Old: `verify_signed_vote_extension(&self, extension, signature, public_key) -> Result<VerificationResult, Error>`
  - New: `verify_signed_vote_extension(&self, scope: &VoteExtensionScope<Ctx>, extension, signature, public_key) -> Result<VerificationResult, Error>`
- Added `VerifierExt::verify_extended_commit_certificate` as a required method. It validates each precommit signature against the reconstructed precommit, each attached vote extension against its precommit scope, and enforces the 2/3+ voting-power quorum. It now takes a `VoteExtensionPolicy` argument, which controls whether vote extensions are required or rejected.

### `malachitebft-core-driver`

- The `Driver` now stores `ExtendedCommitCertificate<Ctx>` instead of `CommitCertificate<Ctx>`. `Driver::commit_certificate(round, value_id)` and `Driver::commit_certificates()` return references to the extended type. Callers that need the bare commit certificate should project via `extended.trim_vote_extensions()`. `Input::CommitCertificate` likewise carries the extended type.

### `malachitebft-core-consensus`

- `Effect::VerifyVoteExtension` now carries an extra `Ctx::Address` field (the precommit's validator) so the verify path can reconstruct the new `VoteExtensionScope`. Custom effect handlers that match on this variant must add the address field.
- Renamed `Effect::ValidSyncValue` → `Effect::CertVerifiedSyncValue` and `Effect::InvalidSyncValue` → `Effect::CertRejectedSyncValue` (fields unchanged). The names now describe the commit-certificate gate they fire on, not the value's validity. Custom effect handlers matching these variants must update the names.
- Added `Effect::VerifyExtendedCommitCertificate` variant. Effect handlers that match exhaustively must add this arm and pass through its `VoteExtensionPolicy` argument.
- `Input::StartHeight` and `State::reset_and_start_height` now carry the height's `VoteExtensionPolicy`. Core integrations should pass `VoteExtensionPolicy::Disabled` for legacy heights and `VoteExtensionPolicy::Required` once vote extensions are mandatory.
- `Error::InvalidCommitCertificate` now carries `ExtendedCommitCertificate<Ctx>` instead of `CommitCertificate<Ctx>`. Pattern matches must adjust.
- `FullProposalKeeper::store_proposal` takes a new `cap_exempt: bool` argument, which admits the proposal even when the `(height, round)` bucket already holds `MAX_PROPOSALS_PER_ROUND` entries. The keeper holds no certificates of its own, so the caller decides: pass `true` when the value holds a polka certificate at that round, and `false` otherwise to keep the previous behavior.

### `malachitebft-engine`

- `RawDecidedBlock<Ctx>.certificate` is now `ExtendedCommitCertificate<Ctx>` instead of `CommitCertificate<Ctx>`.
- Added new `NetworkMsg::CancelRequest(OutboundRequestId)` variant on the network actor, used by sync to drop abandoned outbound requests. Custom network actor implementations that match `NetworkMsg` exhaustively must handle the new variant.
- Changed `NodeRef` from `ActorRef<()>` to `ActorRef<NodeMsg>` to support the new safety-hang signaling path; downstream consumers that pass a `NodeRef` need to update type signatures accordingly
- Removed the `Ctx` type parameter from the `Node` struct (the Node supervisor is no longer generic); `Node::new` and `Node::spawn` no longer require a `Ctx` annotation
- Added new `NodeMsg` enum with a `SafetyFailure(String)` variant, cast by child actors (e.g. the WAL worker thread on panic, the Consensus actor on runtime WAL errors) to signal safety-critical failures to the Node supervisor
- Changed `HostMsg::ProcessSyncedValue` reply type from `Option<ProposedValue<Ctx>>` to the new `SyncedValueOutcome<Ctx>` enum (`Verdict(ProposedValue<Ctx>)` / `PeerFault` / `LocalTransientError`). Replying `None` previously conflated a peer fault with a local/transient failure; hosts must now return the explicit outcome.
- Renamed the sync actor `Msg::InvalidValue(PeerId, Height)` → `Msg::PeerFault(PeerId, Height)` and `Msg::ValueProcessingError(PeerId, Height)` → `Msg::LocalTransientError(Height)` (the peer argument is dropped from the no-blame variant).

### `malachitebft-config`

- Removed the `ValuePayload::PartsOnly` variant and changed the default `value_payload` from `parts-only` to `proposal-and-parts`. Existing configs containing `value_payload = "parts-only"` will fail to deserialize and must be updated.
- Added new `ChannelNames` struct (with `String` fields and a `validate()` method that enforces non-empty, pairwise-unique names) and a `channel_names: ChannelNames` field on `P2pConfig` (opt-in via `#[serde(default)]`). Applications can now configure the GossipSub topic / broadcast channel names from TOML.

### `malachitebft-app-channel`

- Changed `AppMsg::ProcessSyncedValue` reply type from `Option<ProposedValue<Ctx>>` to `SyncedValueOutcome<Ctx>` (`Verdict` / `PeerFault` / `LocalTransientError`); applications must reply with the explicit outcome instead of `Some(value)` / `None`.

### `malachitebft-test-store`

- `DecidedValue.certificate` and `Store::store_decided_value(...)` now take/return `ExtendedCommitCertificate<TestContext>` instead of `CommitCertificate<TestContext>`, so the producer persists vote extensions alongside the decided value and can serve them via sync. The on-disk encoding changes accordingly (proto `ExtendedCommitCertificate`).

### `malachitebft-sync`

- `RawDecidedValue<Ctx>.certificate` is now `ExtendedCommitCertificate<Ctx>` instead of `CommitCertificate<Ctx>`. For protobuf-encoded sync messages or records, the new certificate preserves the previous field layout and adds optional vote-extension fields, so old and new binaries can decode each other's certificates while vote extensions remain disabled. Once an application starts a height with `VoteExtensionPolicy::Required`, all validators and sync-serving nodes must be upgraded first because old binaries cannot produce the required extensions. Borsh-encoded sync messages or records are not wire-compatible across this change.
- The `borsh`/`proto` codec entries for the sync wire have been updated accordingly. Proto messages `CommitCertificate` and `CommitSignature` in `sync.proto` are replaced by `ExtendedCommitCertificate` and `ExtendedCommitSignature`; they keep the same core field numbers, and `ExtendedCommitSignature` adds an optional `Extension`.
- Added new `Effect::CancelValueRequest(OutboundRequestId, resume::Continue)` variant, emitted on sync request timeout so the network layer can drop the abandoned in-flight request. Custom effect handlers that match `Effect` exhaustively must handle the new variant.
- Renamed the `Input` variants `InvalidValue(PeerId, Height)` → `PeerFault(PeerId, Height)` and `ValueProcessingError(PeerId, Height)` → `LocalTransientError(Height)` (the peer argument is dropped from the no-blame variant, which now re-requests without penalizing or excluding any peer).
- Added an `inflight` field to `PendingRequestEntry`, set to `false` once the response for the range has arrived. Only in-flight entries count against `parallel_requests`. Code that builds this struct literally must set the new field, and code that reads `pending_requests.len()` as a measure of outstanding requests should use `State::inflight_requests()` instead. `State::update_request` takes a matching `inflight` argument.

### `malachitebft-network`

- `ChannelNames` fields are now owned `String`s instead of `&'static str`. The struct no longer implements `Copy`, and the following methods on `Channel` now take `&ChannelNames` by reference:
  - `Channel::to_gossipsub_topic`, `Channel::to_broadcast_topic`, `Channel::as_str`
  - `Channel::has_gossipsub_topic`, `Channel::has_broadcast_topic`
  - `Channel::from_gossipsub_topic_hash`, `Channel::from_broadcast_topic`
- Removed the `validator_proof::Event::ProofReceiveFailed` variant and the `validator_proof::Error::UnexpectedEof` variant. Malformed or failed inbound proofs now disconnect the peer directly inside the behaviour, so neither is emitted.

## 0.7.0

*June 22nd, 2026*

> [!IMPORTANT]
> All crates were renamed from `informalsystems-malachitebft-$crate` to `arc-malachitebft-$crate`.

### `malachitebft-core-types`

- Added new `ValidatorProof<Ctx>` type for the Proof-of-Validator protocol (ADR-006)
- Added new associated type `Timeouts` to the `Context` trait (use `LinearTimeouts` for default implementation) ([#1227](https://github.com/circlefin/malachite/pull/1227))
- Remove `initial_validator_set` and `initial_height` fields from `Params` struct ([#1190](https://github.com/circlefin/malachite/pull/1190))

### `malachitebft-signing`

- Split `SigningProvider` into two independent traits:
  - `Verifier<Ctx>` — signature verification (no key material required)
  - `Signer<Ctx>` — message signing (requires private key)
- `SigningProviderExt` has been removed; use `VerifierExt` directly
- APIs that previously took `Box<dyn SigningProvider<Ctx>>` now take `Box<dyn Verifier<Ctx>>` and/or `Box<dyn Signer<Ctx>>` separately
- Removed `Signer::sign_bytes` and `Verifier::verify_signed_bytes`; every signing purpose is now a named trait method. Previously these were untyped blob channels that forced downstream signers to inspect bytes or maintain ambient state to pick a domain.
- Added `sign_validator_proof` as a required method on the `Signer` trait for Proof-of-Validator (ADR-006):
  - `sign_validator_proof(&self, public_key: Vec<u8>, peer_id: Vec<u8>) -> Result<ValidatorProof<Ctx>, Error>`
- Added `verify_validator_proof` as a required method on the `Verifier` trait for Proof-of-Validator (ADR-006):
  - `verify_validator_proof(&self, proof: &ValidatorProof<Ctx>) -> Result<VerificationResult, Error>`
- Removed the `SignerExt` trait; `sign_validator_proof` now lives on `Signer` directly. Migrate downstream `use … SignerExt` imports to `use … Signer`.

### `malachitebft-core-driver`

- Changed `Driver::new()` signature - removed `timeouts` parameter ([#1227](https://github.com/circlefin/malachite/pull/1227))
  - Old: `Driver::new(ctx, height, validator_set, timeouts, address, threshold_params)`
  - New: `Driver::new(ctx, height, validator_set, address, threshold_params)`
- Removed `Driver::timeouts()` method - timeouts are now accessed through `State` instead ([#1227](https://github.com/circlefin/malachite/pull/1227))
- Removed `timeouts` field from `Driver` struct - Driver no longer stores or manages timeouts ([#1227](https://github.com/circlefin/malachite/pull/1227))
- Changed `Driver::move_to_height` signature from `move_to_height(Height, Validator_set, Timeouts)` to `move_to_height(Height, Option<ValidatorSet>)` ([#1227](https://github.com/circlefin/malachite/pull/1227))

### `malachitebft-core-consensus`

- Removed `Effect::ResetTimeouts` variant ([#1227](https://github.com/circlefin/malachite/pull/1227))
- Changed `Input::StartHeight` from `StartHeight(Height, ValidatorSet, bool)` to `StartHeight(Height, Option<ValidatorSet>, bool)` ([#1227](https://github.com/circlefin/malachite/pull/1227))
- Changed `State::reset_and_start_height()` signature from `(height, validator_set)` to `(height, validator_set: Option<ValidatorSet>)` ([#1227](https://github.com/circlefin/malachite/pull/1227))

### `malachitebft-engine`

- Added new `NetworkEvent::ValidatorProofReceived` variant for receiving validator proofs (ADR-006)
- Added new `Msg::ValidatorProofVerified` variant for communicating proof verification results
- Network codec trait bounds now require `Codec<ValidatorProof<Ctx>>` implementation
- Changed `Next::Start` variant from `Start(Height, ValidatorSet)` to `Start(Height, HeightParams)` ([#1227](https://github.com/circlefin/malachite/pull/1227))
- Changed `Next::Restart` variant from `Restart(Height, ValidatorSet)` to `Restart(Height, HeightParams)` ([#1227](https://github.com/circlefin/malachite/pull/1227))
- Changed `HostMsg::ConsensusReady` reply type from `(Ctx::Height, Ctx::ValidatorSet)` to `(Ctx::Height, HeightParams<Ctx>)` ([#1227](https://github.com/circlefin/malachite/pull/1227))
- Changed `Msg::StartHeight` from `StartHeight(Height, ValidatorSet)` to `StartHeight(Height, HeightParams)` ([#1227](https://github.com/circlefin/malachite/pull/1227))
- Changed `Msg::RestartHeight` from `RestartHeight(Height, ValidatorSet)` to `RestartHeight(Height, HeightParams)` ([#1227](https://github.com/circlefin/malachite/pull/1227))
- Added `timeouts` field to `State` struct - timeouts are now stored in State instead of Driver ([#1227](https://github.com/circlefin/malachite/pull/1227))
- Added `WalEntry::PolkaCertificate` variant - polka certificates are now stored in the WAL; if recovering with downgraded version fails, restart with the new version

### `malachitebft-config`

- Removed `TimeoutConfig` struct ([#1227](https://github.com/circlefin/malachite/pull/1227))
- Removed `timeouts` field from `ConsensusConfig` struct (timeouts are now managed via `Context::Timeouts` associated type) ([#1227](https://github.com/circlefin/malachite/pull/1227))

### `malachitebft-app-channel`

- Changed `NetworkContext` struct for Proof-of-Validator support (ADR-006):
  - Old: `NetworkContext::new(identity: NetworkIdentity, codec: Codec)` (where `NetworkIdentity` had no proof)
  - New: `NetworkContext::new(identity: NetworkIdentity, codec: Codec)` (where `NetworkIdentity` carries pre-signed proof bytes)
  - The application is now responsible for signing the validator proof and building `NetworkIdentity::new_validator(moniker, keypair, address, proof_bytes)` before passing it to `NetworkContext`
- Re-exported `Signer` and `Verifier` from `malachitebft_app_channel`; apps call `sign_validator_proof` / `verify_validator_proof` directly on the primary traits now
- `ConsensusContext` now has two constructors: `new_validator(address, verifier, signer)` for validators and `new_full_node(address, verifier)` for non-validator nodes
- Network codec now requires `Codec<ValidatorProof<Ctx>>` implementation
- Changed `AppMsg::ConsensusReady` reply type from `(Ctx::Height, Ctx::ValidatorSet)` to `(Ctx::Height, HeightParams<Ctx>)` ([#1227](https://github.com/circlefin/malachite/pull/1227))
- Changed `ConsensusMsg::StartHeight` from `StartHeight(Height, ValidatorSet)` to `StartHeight(Height, HeightParams)` ([#1227](https://github.com/circlefin/malachite/pull/1227))
- Changed `ConsensusMsg::RestartHeight` from `RestartHeight(Height, ValidatorSet)` to `RestartHeight(Height, HeightParams)` ([#1227](https://github.com/circlefin/malachite/pull/1227))

### `malachitebft-app`

- Removed `Node` trait

### `malachitebft-sync`

- Added new `PartialSuccess { received, requested, response_time }` variant to `SyncResult`. Custom implementations of `ScoringStrategy` that match on `SyncResult` must handle the new variant.

### `malachitebft-engine-byzantine`

- Removed `ByzantineMiddleware`. The `TestContext`-specific middleware has been relocated to `malachitebft_test::byzantine::ByzantineMiddleware`. Downstream contexts that want the same behavior should embed the new `Amnesia<Ctx>` tracker into their own prevote-construction hook.
- Added `Amnesia<Ctx>` context-generic amnesia state machine, exposed at the crate root.
- Removed `malachitebft-test` from regular dependencies (now dev-only). Consumers no longer transitively pull in the test crate.

### `malachitebft-test`

- `ByzantineMiddleware` now lives under `malachitebft_test::byzantine` (previously at `malachitebft_engine_byzantine::ByzantineMiddleware`). Its constructor takes 5 args `(ignore_locks, force_precommit_nil, inner, self_address, seed)` and internally delegates to `Amnesia<TestContext>`.

### `malachitebft-app-channel`

- Added optional `byzantine` Cargo feature that enables `EngineBuilder::with_byzantine_network` and the `ByzantineContext` input struct. Off by default; enabling it adds `malachitebft-engine-byzantine` as a transitive dependency.

## 0.6.0

### `malachitebft-core-types`

- Move `SigningProvider` and `SigningProviderExt` traits into new `malachitebft-signing` crate ([#1191](https://github.com/circlefin/malachite/pull/1191))

### `malachitebft-signing`

- New crate exposing the `SigningProvider` trait ([#1191](https://github.com/circlefin/malachite/pull/1191))
- Make methods of `SigningProvider` and `SigningProviderExt` traits fallible ([#1191](https://github.com/circlefin/malachite/pull/1191))
- Changed methods of `SigningProvider` and `SigningProviderExt` traits to `async` ([#1151](https://github.com/circlefin/malachite/issues/1151))

### `malachitebft-core-consensus`

- Remove `GetValidatorSet` effect ([#1189](https://github.com/circlefin/malachite/pull/1189))

### `malachitebft-engine`

- Remove `HostMsg::GetValidatorSet` ([#1189](https://github.com/circlefin/malachite/pull/1189))

### `malachitebft-config`

- Added field `channel_names: ChannelNames` to `NetworkConfig` struct ([#849](https://github.com/circlefin/malachite/pull/849))

### `malachitebft-app-channel`

- Remove `AppMsg::GetValidatorSet` ([#1189](https://github.com/circlefin/malachite/pull/1189))
- Added field `requests: tokio::sync::mpsc::Sender<ConsensusRequest<Ctx>>` to `Channels` struct ([#1176](https://github.com/circlefin/malachite/pull/1176))

## 0.5.0

*July 31st, 2025*

### General

- Updated libp2p to v0.56.x ([#1124](https://github.com/circlefin/malachite/pull/1124))

### `malachitebft-app-channel`

- Changed type of field `reply` of enum variant `AppMsg::Decided` to `Reply<malachitebft_engine::host::Next<Ctx>>` ([#1109](https://github.com/circlefin/malachite/pull/1109))

### `malachitebft-engine`

- Changed tuple field of enum variant `HostMsg::ConsensusReady` to a field named `reply_to` of type `RpcReplyPort<(Ctx::Height, Ctx::ValidatorSet)>` ([#1109](https://github.com/circlefin/malachite/pull/1109))
- Added field `reply_to` to enum variant `HostMsg::StartedRound` with type `RpcReplyPort<Vec<ProposedValue<Ctx>>>` ([#1109](https://github.com/circlefin/malachite/pull/1109))
- Changed type of field `reply_to` of enum variant `HostMsg::Decided` to `RpcReplyPort<malachitebft_engine::host::Next<Ctx>>` ([#1109](https://github.com/circlefin/malachite/pull/1109))

### `malachitebft-core-consensus`

- Rename `Effect::RebroadcastVote` to `Effect::RepublishVote` and `Effect::RebroadcastRoundCertificate` to `Effect::RepublishRoundCertificate` ([#1011](https://github.com/circlefin/malachite/issues/1011))
- Add new `Effect::SyncValue` variant to forward synced values to the application ([#1149](https://github.com/circlefin/malachite/pull/1149))

### `malachitebft-sync`

#### Enum Changes

- Renamed `GetDecidedValue` to `GetDecidedValues` in `Effect`.
  - Now it takes a range of heights instead of one, and the reply is a list (possibly empty) of
    decided values instead of one or zero.
- Renamed `GotDecidedValue` to `GotDecidedValues` in `Msg` and `Input`.
  - Now it has as parameter a range of heights instead of one, and a list of decided values instead
    of one or zero.
- Added new parameter to `SyncRequestTimedOut` in `Input`.
- Renamed `Effect::RebroadcastVote` to `Effect::RepublishVote` and `Effect::RebroadcastRoundCertificate` to `Effect::RepublishRoundCertificate` ([#1011](https://github.com/circlefin/malachite/issues/1011))
- Added new `Effect::SyncValue` variant to forward synced values to the application ([#1149](https://github.com/circlefin/malachite/pull/1149))
- Removed `Input::CommitCertificate` variant ([#1149](https://github.com/circlefin/malachite/pull/1149))
- Added new `Input::SyncValueResponse` variant to notify consensus of a sync value having been received via the sync protocol ([#1149](https://github.com/circlefin/malachite/pull/1149))

## 0.4.0

*July 8th, 2025*

### `malachitebft-config`
- Added new sync parameters to config.
  See ([#1092](https://github.com/circlefin/malachite/issues/1092)) for more details.

### `malachitebft-sync`
- Added new parallel requests related parameters to sync config.
  See ([#1092](https://github.com/circlefin/malachite/issues/1092)) for more details.


## 0.3.1

*July 7th, 2025*

No breaking changes.


## 0.3.0

*June 17th, 2025*

### `malachitebft-core-types`
- Removed the VoteSet synchronization protocol, as it is neither required nor sufficient for liveness.
  See ([#998](https://github.com/circlefin/malachite/issues/998)) for more details.

### `malachitebft-core-consensus`
- Removed the VoteSet synchronization protocol, as it is neither required nor sufficient for liveness.
  See ([#998](https://github.com/circlefin/malachite/issues/998)) for more details.
- Added new variants to `Input` enum: `PolkaCertificate` and `RoundCertificate`
- Added new variant to `Effect` enum: `PublishLivenessMessage`

### `malachitebft-metrics`
- Removed app-specific metrics from the `malachitebft-metrics` crate ([#1054](https://github.com/circlefin/malachite/issues/1054))

### `malachitebft-engine`
- Removed the VoteSet synchronization protocol, as it is neither required nor sufficient for liveness.
  See ([#998](https://github.com/circlefin/malachite/issues/998)) for more details.
- Changed the reply channel of `GetValidatorSet` message to take an `Option<Ctx::ValidatorSet>` instead of `Ctx::ValidatorSet`.
- Added new variant to `Msg` enum: `PublishLivenessMsg`
- Added new variants to `NetworkEvent` enum: `PolkaCertificate` and `RoundCertificate`
- Changed `PartStore::all_parts` to `PartStore::all_parts_by_stream_id`:
  - Renamed method to clarify that, when a new part is received, the contiguous parts should be queried by stream id
  - Added required `StreamId` parameter
- Added new public API `PartStore::all_parts_by_value_id` to be used instead of `PartStore::all_parts` when a decision is reached
- Added `&StreamId` parameter to `part_store::PartStore::store`
- Added `&StreamId` parameter to `part_store::PartStore::store_value_id`
- Changed semantics of `RestreamProposal` variant of `HostMsg`: the value at `round` should be now be restreamed if `valid_round` is `Nil`

### `malachitebft-network`
- Added new variant to `Channel` enum: `Liveness`
- Renamed `Event::Message` variant to `Event::ConsensusMessage`
- Added new variant to `Event::LivenessMessage`

### `malachitebft-sync`
- Removed the VoteSet synchronization protocol, as it is neither required nor sufficient for liveness.
  See ([#998](https://github.com/circlefin/malachite/issues/998)) for more details.

### `arc-malachitebft-app-channel`
- The `start_engine` function now takes two `Codec`s: one for the WAL and one for the network.

## 0.2.0

### `malachitebft-core-types`
- Remove `AggregatedSignature` type
- Rename field `aggregated_signature` of `CommitCertificate` to `commit_signatures`
- Remove field `votes` of `PolkaCertificate`
- Add field `polka_signatures` to `PolkaCertificate`
- Rename `InvalidSignature` variant of `CertificateError` to `InvalidCommitSignature`
- Add `InvalidPolkaSignature` and `DuplicateVote` variants to `CertificateError`
- Remove `verify_commit_signature` from `SigningProvider`

### `malachitebft-core-consensus`
- Add `VerifyPolkaCertificate` effect
- Rename `Effect::VerifyCertificate` to `Effect::VerifyCommitCertificate`
- Rename `Error::InvalidCertificate` to `Error::InvalidCommitCertificate`

## 0.1.0

### `malachitebft-core-types`

#### Enum Changes
- Added new variants to `TimeoutKind` enum: `PrevoteRebroadcast` and `PrecommitRebroadcast`.

#### Struct Changes
- Removed the `Extension` struct that was previously available at `arc_malachitebft_core_types::Extension`.
- Removed the `extension` field from the `CommitSignature` struct.
- Changed `CommitSignature::new()` method to take 2 parameters instead of 3.

#### Trait Changes
- Added associated constants to `Height` trait without default values:
  - `Height::ZERO`
  - `Height::INITIAL`

- Added new associated type to `Context` trait without a default value:
  - `Context::Extension`

- Removed associated type `Context::SigningProvider`

- Added new methods to `SigningProvider` trait without default implementations:
  - `sign_vote_extension`
  - `verify_signed_vote_extension`

- Removed method `signing_provider` from `Context` trait

- Changed parameter count for these `Context` trait methods:
  - `new_proposal`: now takes 6 parameters instead of 5
  - `new_prevote`: now takes 5 parameters instead of 4
  - `new_precommit`: now takes 5 parameters instead of 4

### `malachitebft-core-consensus`

#### Struct Changes
- Added new fields to externally-constructible structs:
  - `State.last_signed_prevote`
  - `State.last_signed_precommit`
  - `State.decided_sent`
  - `Params.vote_sync_mode`

- Removed public fields from structs:
  - Removed `extension` field from `ProposedValue`
  - Removed `signed_precommits` field from `State`
  - Removed `decision` field from `State`

- Removed structs:
  - `ValueToPropose` has been removed

#### Enum Changes
- Removed enums:
  - `ValuePayload` has been completely removed

- Added new variants to existing enums:
  - Added to `Error`: `DecisionNotFound`, `DriverProposalNotFound`, `FullProposalNotFound`
  - Added to `Effect`: `Rebroadcast`, `RestreamProposal`, `RequestVoteSet`, `WalAppend`, `ExtendVote`, `VerifyVoteExtension`
  - Added to `Resume`: `VoteExtension`, `VoteExtensionValidity`

- Removed variants from enums:
  - Removed from `Error`: `DecidedValueNotFound`
  - Removed from `Effect`: `RestreamValue`, `GetVoteSet`, `PersistMessage`, `PersistTimeout`

- Modified enum tuple variants by adding fields:
  - Added field to `Input::VoteSetResponse`
  - Added field to `Effect::Decide`
  - Added field to `Effect::SendVoteSetResponse`

#### Method Changes
- Removed methods:
  - `State::store_signed_precommit`
  - `State::store_decision`
  - `State::full_proposals_for_value`
  - `State::remove_full_proposals`


### `arc-malachitebft-sync`

#### Struct Changes
- Added new field to externally-constructible struct:
  - `VoteSetResponse.polka_certificates`

- Removed struct:
  - `DecidedValue` has been completely removed

#### Enum Changes
- Added new variant to existing enum:
  - Added to `Effect`: `GetDecidedValue`

- Removed variant from enum:
  - Removed from `Effect`: `GetValue`

#### Method Changes
- Changed parameter count:
  - `VoteSetResponse::new` now takes 4 parameters instead of 3

### `arc-malachitebft-engine`

#### Enum Changes
- Removed enums:
  - `WalEntry` has been completely removed from the `wal` module

- Added new variants to existing enums:
  - Added to `Msg`: `Dump`
  - Added to `Event`: `Rebroadcast`, `WalReplayEntry`, `WalReplayError`
  - Added to `HostMsg`: `ExtendVote`, `VerifyVoteExtension`, `PeerJoined`, `PeerLeft`

- Removed variants from enums:
  - Removed from `Msg`: `GetStatus`
  - Removed from `Event`: `WalReplayConsensus`, `WalReplayTimeout`

- Modified enum variants:
  - Added field `listen_addrs` to struct variant `State::Running`
  - Added field `extensions` to struct variant `HostMsg::Decided`
  - Changed variant `StreamContent::Fin` to a different kind
  - Added field to tuple variant `Event::SentVoteSetResponse`
  - Removed multiple fields from tuple variant `Msg::ProposeValue`

#### Method Changes
- Changed parameter count:
  - `Node::new` now takes 7 parameters instead of 9
  - `Consensus::spawn` now takes 11 parameters instead of 10

#### Struct Changes
- Removed struct:
  - `LocallyProposedValue` has been removed from the `host` module

### `arc-malachitebft-app-channel`

#### Struct Changes
- Added new fields to externally-constructible structs:
  - Added `events` field to `Channels`
  - Added `reply_value` field to `AppMsg::StartedRound` variant
  - Added `extensions` field to `AppMsg::Decided` variant

- Added new variants to existing enums:
  - Added to `ConsensusMsg`: `ReceivedProposedValue`
  - Added to `AppMsg`: `ExtendVote`, `VerifyVoteExtension`, `PeerJoined`, `PeerLeft`

#### Function Renames
  - `run` is now called `start_engine`
