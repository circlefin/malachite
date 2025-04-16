# Breaking Changes

## Unreleased

### `malachitebft-engine`
- Changed the reply channel of `GetValidatorSet` message to take an `Option<Ctx::ValidatorSet>` instead of `Ctx::ValidatorSet`.

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

