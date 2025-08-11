# Breaking Changes

## Unreleased

### `malachitebft-core-types`

- Changed methods of `SigningProvider` and `SigningProviderExt` traits to `async` ([#1151](https://github.com/informalsystems/malachite/issues/1151))

### `malachitebft-config`

- Added field `channel_names: ChannelNames` to `NetworkConfig` struct ([#849](https://github.com/informalsystems/malachite/pull/849))


## 0.5.0

*July 31st, 2025*

### General

- Updated libp2p to v0.56.x ([#1124](https://github.com/informalsystems/malachite/pull/1124))

### `malachitebft-app-channel`

- Changed type of field `reply` of enum variant `AppMsg::Decided` to `Reply<malachitebft_engine::host::Next<Ctx>>` ([#1109](https://github.com/informalsystems/malachite/pull/1109))

### `malachitebft-engine`

- Changed tuple field of enum variant `HostMsg::ConsensusReady` to a field named `reply_to` of type `RpcReplyPort<(Ctx::Height, Ctx::ValidatorSet)>` ([#1109](https://github.com/informalsystems/malachite/pull/1109))
- Added field `reply_to` to enum variant `HostMsg::StartedRound` with type `RpcReplyPort<Vec<ProposedValue<Ctx>>>` ([#1109](https://github.com/informalsystems/malachite/pull/1109))
- Changed type of field `reply_to` of enum variant `HostMsg::Decided` to `RpcReplyPort<malachitebft_engine::host::Next<Ctx>>` ([#1109](https://github.com/informalsystems/malachite/pull/1109))

### `malachitebft-core-consensus`

- Rename `Effect::RebroadcastVote` to `Effect::RepublishVote` and `Effect::RebroadcastRoundCertificate` to `Effect::RepublishRoundCertificate` ([#1011](https://github.com/informalsystems/malachite/issues/1011))
- Add new `Effect::SyncValue` variant to forward synced values to the application ([#1149](https://github.com/informalsystems/malachite/pull/1149))

### `malachitebft-sync`

#### Enum Changes

- Renamed `GetDecidedValue` to `GetDecidedValues` in `Effect`. 
  - Now it takes a range of heights instead of one, and the reply is a list (possibly empty) of
    decided values instead of one or zero.
- Renamed `GotDecidedValue` to `GotDecidedValues` in `Msg` and `Input`. 
  - Now it has as parameter a range of heights instead of one, and a list of decided values instead
    of one or zero.
- Added new parameter to `SyncRequestTimedOut` in `Input`.
- Renamed `Effect::RebroadcastVote` to `Effect::RepublishVote` and `Effect::RebroadcastRoundCertificate` to `Effect::RepublishRoundCertificate` ([#1011](https://github.com/informalsystems/malachite/issues/1011))
- Added new `Effect::SyncValue` variant to forward synced values to the application ([#1149](https://github.com/informalsystems/malachite/pull/1149))
- Removed `Input::CommitCertificate` variant ([#1149](https://github.com/informalsystems/malachite/pull/1149))
- Added new `Input::SyncValueResponse` variant to notify consensus of a sync value having been received via the sync protocol ([#1149](https://github.com/informalsystems/malachite/pull/1149))

## 0.4.0

*July 8th, 2025*

### `malachitebft-config`
- Added new sync parameters to config.
  See ([#1092](https://github.com/informalsystems/malachite/issues/1092)) for more details.

### `malachitebft-sync`
- Added new parallel requests related parameters to sync config.
  See ([#1092](https://github.com/informalsystems/malachite/issues/1092)) for more details.


## 0.3.1

*July 7th, 2025*

No breaking changes.


## 0.3.0

*June 17th, 2025*

### `malachitebft-core-types`
- Removed the VoteSet synchronization protocol, as it is neither required nor sufficient for liveness.
  See ([#998](https://github.com/informalsystems/malachite/issues/998)) for more details.

### `malachitebft-core-consensus`
- Removed the VoteSet synchronization protocol, as it is neither required nor sufficient for liveness.
  See ([#998](https://github.com/informalsystems/malachite/issues/998)) for more details.
- Added new variants to `Input` enum: `PolkaCertificate` and `RoundCertificate`
- Added new variant to `Effect` enum: `PublishLivenessMessage`

### `malachitebft-metrics`
- Removed app-specific metrics from the `malachitebft-metrics` crate ([#1054](https://github.com/informalsystems/malachite/issues/1054))

### `malachitebft-engine`
- Removed the VoteSet synchronization protocol, as it is neither required nor sufficient for liveness.
  See ([#998](https://github.com/informalsystems/malachite/issues/998)) for more details.
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
  See ([#998](https://github.com/informalsystems/malachite/issues/998)) for more details.

### `informalsystems-malachitebft-app-channel`
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
- Removed the `Extension` struct that was previously available at `informalsystems_malachitebft_core_types::Extension`.
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


### `informalsystems-malachitebft-sync`

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

### `informalsystems-malachitebft-engine`

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

### `informalsystems-malachitebft-app-channel`

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

