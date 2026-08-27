# Release Notes

## Unreleased

## 0.8.0

*August 27th, 2026*

### `app-channel`
- Fix `build()` leaking the Node actor when a post-Node spawn fails; the Node (and any children linked to it so far) are now stopped before the error is returned
- Change the `AppMsg::ProcessSyncedValue` reply from `Option<ProposedValue>` to `SyncedValueOutcome` (`Verdict` / `PeerFault` / `LocalTransientError`) so applications can distinguish a peer-attributable fault from a local/transient one

### `consensus`
- Cap per-round consensus timeouts via a new `max_timeout` field on `LinearTimeouts` (default 60s) so `duration_for` cannot grow without bound at high round numbers
- Remove the `PartsOnly` value-propagation mode; the default is now `ProposalAndParts`. Applications carrying `Proposal` metadata in the `Init` part must migrate to `ProposalAndParts` or maintain a fork.
- Emit `Effect::Finalize` before resetting state when `Input::StartHeight` arrives during the finalization window, so the commit certificate and equivocation evidence are not silently dropped
- Persist the votes of a round certificate to the WAL after verification, so a node that crashes mid-round re-aggregates them on restart and recovers the certificate without re-fetching it from peers
- Rename `Effect::ValidSyncValue` / `InvalidSyncValue` to `CertVerifiedSyncValue` / `CertRejectedSyncValue` to reflect that they gate on the commit-certificate check, not the value's validity
- Exempt polka-certified values from the per-round proposal cap. A restreamed proposal carries its original round, so a round whose cap was already filled with equivocating values could permanently reject the one value the network had polka'd, at that round and every later one. This also left the hidden-lock liveness backstop unable to fire. A polka certificate carries a quorum of signed prevotes, so at most one value per round qualifies and the flood bound still holds for uncertified entries

### `engine`
- Add split safety/liveness supervisor policy on the Node actor, routing failures to one of two recovery paths based on what the failure means. Liveness failures (Host/Network/Sync crash, startup-path WAL errors) stop the Node so the orchestrator restarts the process; safety-critical failures (WAL worker thread panic, runtime `wal_append` / `wal_flush` errors) hang the Node for operator inspection to prevent auto-restart from double-signing on top of an incomplete WAL
- Add `node_safety_failure` gauge, flipped to `1` when the Node enters the safety-hang state
- Fix runtime `wal_append` / `wal_flush` errors being swallowed at `Effect::WalAppend`, `Effect::PublishConsensusMsg`, `Effect::Decide`, and `Effect::StartRound` — consensus could previously broadcast votes the WAL did not durably record, risking a double-sign on restart
- Fix WAL worker thread panics disappearing silently into a logged error; the worker now casts `NodeMsg::SafetyFailure` before exiting so the Node enters safety-hang instead of auto-restarting on top of unknown WAL state
- Fix `stop_on_failure` deadlock: the helper no longer calls `pending()` after `myself.stop()` (which never resolved from inside the calling actor's own handler); it returns `Result<A, ActorProcessingErr>` so callers `?`-propagate out of `handle`, which fails the actor (`ActorFailed`) and lets the Node supervisor restart the process
- Model the `HostMsg::ProcessSyncedValue` reply as an explicit `SyncedValueOutcome` (`Verdict` / `PeerFault` / `LocalTransientError`) instead of `Option<ProposedValue>`, so a local/transient host failure is no longer conflated with a peer fault and routed into a peer penalty
- Add `NetworkMsg::CancelRequest(OutboundRequestId)` so the consensus layer
  can ask the network actor to drop an abandoned outbound sync request. The
  bundled libp2p network actor logs and no-ops (no public cancel API in
  `libp2p::request_response`); downstream network actors that own the
  transport can use this to free transport resources eagerly

### `driver`
- Add `IntoIterator` impls and `len()` to `EvidenceMap` in `core-driver` and `core-votekeeper`

### `network`
- Support peer-only multiaddrs (`/p2p/<peer_id>`) in `persistent_peers`: entries without a transport component are used for inbound identity filtering and are never dialed
- Make GossipSub topic / broadcast channel names configurable via `P2pConfig.channel_names`; channel names are validated for non-emptiness and uniqueness before the network actor is spawned

### `signing`
- Bind vote-extension signatures to their precommit scope `(height, round, value_id, validator_address)` so an extension blob cannot be relayed across heights, rounds, values, or validators
- Add `ExtendedCommitCertificate<Ctx>`, a self-verifiable bundle of per-validator precommit signatures and their optional vote extensions, with constructors that rebuild it from raw votes (`from_votes`) or from the host API's parallel `(CommitCertificate, VoteExtensions)` pair (`from_commit_certificate_and_extensions`). Verify the whole bundle in one pass via `VerifierExt::verify_extended_commit_certificate`
- Sync now carries vote extensions: `ValueResponse`, `RawDecidedValue`, `RawDecidedBlock`, and the on-disk `DecidedValue` all hold `ExtendedCommitCertificate` so a node catching up via sync can propose the next height when the application uses extensions for load-bearing data. Applications choose per height whether extensions must be absent or present via `HeightParams::with_vote_extension_policy(VoteExtensionPolicy::{Disabled, Required})`. Closes the second half of  (proposer-after-sync corner case).

### `sync`
- Count only requests awaiting a response against `parallel_requests`. A response that has arrived leaves a reservation in `pending_requests`, so its range is not requested twice, but it no longer consumes a request slot. Such reservations could previously fill the whole budget above a height that had no request left. They can neither be pruned (consensus decides in order) nor time out (their response arrived), so catch-up stalled until the serving peer disconnected or the process restarted. New batches do not start more than `parallel_requests * batch_size` heights above the tip; the final batch can extend by up to `batch_size - 1` additional heights. This replaces the implicit read-ahead control from the old slot accounting. This adds the `inflight` field to the public `PendingRequestEntry` and an `inflight` parameter to `State::update_request`
- On peer disconnect, re-request only the ranges still awaiting a response. A reservation already holds its values, so re-requesting it took a request slot and buffered a second copy of every height in its range
- Start a request pass as soon as a full response releases a request slot. The other triggers are a peer status and the start of a height, and neither is guaranteed while a lower height is missing: consensus cannot start a height, and a peer that has stopped deciding broadcasts no further status when `status_update_interval` is `0`
- Schedule the remainder of a partial response from the global frontier. The dedicated suffix scheduler never read `sync_height`, so it could not request a lower uncovered height, and it left every other free request slot idle. A node that lost a low range to retry exhaustion kept fetching suffixes above the gap and never went back for it. With eager status updates no later input reopened the question, so the node stayed at a fixed tip while a connected peer still held every value it needed
- Prune completed partial-response reservations after consensus has already advanced past them, so a full pending-request buffer cannot prevent ValueSync from requesting the next uncovered range
- Emit new `Effect::CancelValueRequest` from `on_sync_request_timed_out`
  before re-requesting, so the network layer can drop the abandoned
  in-flight request instead of letting it complete and trigger downstream
  work (certificate fetches, rate-limit headroom) for a response that will
  be discarded
- Fix the retry path orphaning a range suffix when the only eligible replacement peer can serve just a prefix (lower tip): `sync_height` is now rolled back to the suffix start so the next request cycle re-requests it, instead of stranding those heights below `sync_height` and stalling catch-up
- Re-request a synced value on a local/transient processing failure (e.g. the execution layer being temporarily unavailable) without penalizing or excluding the serving peer, so an outage that fails every peer identically can no longer exhaust the peer set and rewind `sync_height` into a silent stall. Renamed the `InvalidValue` / `ValueProcessingError` sync inputs to `PeerFault` / `LocalTransientError`, dropping the peer argument from the no-blame variant
- Stop the value-sync request loop after a transport-level send failure instead of re-selecting the identical range against an available peer, so an unreachable network layer no longer spins the sync actor; the range is reconsidered on the next request trigger

## 0.7.0

*June 22nd, 2026*

> [!IMPORTANT]
> All crates were renamed from `informalsystems-malachitebft-$crate` to `arc-malachitebft-$crate`.

### `app-channel`
- Add builder pattern for custom actor injection
- Make consensus request channel capacity configurable
- Refactor infrastructure for spawning a channel-based application
- Add `EngineBuilder::with_byzantine_network` hook (behind `byzantine` feature) to inject the Byzantine network proxy

### `consensus`
- Allow application to change its mind about validity (invalid -> valid)
- Add an ability to add/remove persistent peers at runtime via `Network` handle
- Add `persistent_peers_only` config option to allow connections ONLY from/to persistent peers
- Allow dynamic adjustment of timeout parameters ([#1227](https://github.com/circlefin/malachite/pull/1227))
- Allow providing both the validator set and the timeouts for a height in `StartHeight`, `RestartHeight` and `ConsensusReady` reply ([#1227](https://github.com/circlefin/malachite/pull/1227))
- Remove `initial_validator_set` and `initial_height` fields from `Params` struct ([#1190](https://github.com/circlefin/malachite/pull/1190))
- Drop synced values whose id does not match the accompanying commit certificate before forwarding to consensus

### `discovery`
- Can connect request calls the wrong controller action
- Clear connect_request done_on to allow re-upgrading the peer on reconnection
- Don't add peers with empty address list to dial queue
- Don't cancel outgoing dials when receiving inbound connection from same peer
- Ensure discovery configuration is passed down to the networking module
- Fix peer and connection metrics when discovery is disabled
- Prevent address poisoning when discovery is enabled
- Prevent address spoofing in persistent peer detection

### `driver`
- Check for polka certificate to multiplex `PolkaValue` output on step change
- Clear scheduled timeouts when skipping to a higher round
- Ensure `PrecommitAny` does not shadow `PolkaNil` and `PolkaAny` pending inputs
- Ensure polka certificate is matched against a proposal for the same value
- Produce `InvalidProposalAndPolkaPrevious` when receiving a polka certificate matching the POL round of a proposal with an invalid value

### `engine-byzantine`
- Introduce a new crate that simulates Byzantine faults at the engine layer via `ByzantineNetworkProxy` and a context-generic `Amnesia<Ctx>` tracker decoupled from `TestContext`
- Add `force_precommit_nil` and `drop_inbound_proposals` attacks, backed by a new `InboundFilter` actor and an `AtHeightsAndRounds` trigger variant
- Remove the `TestContext`-specific `ByzantineMiddleware` (relocated to `malachitebft_test::byzantine`); `malachitebft-test` is no longer a regular dependency of this crate

### `network`
- Add `persistent_peers_only` config option to allow connections ONLY from/to persistent peers
- Add a mechanism to dump the network state
- Add application-specific peer scoring for Gossipsub to prioritize nodes based on their types, in mesh formation and maintenance
- Add network metrics for peer identification and tracking
- Add transport level connection limits
- Limit the number of peers that can connect from same IP address

### `signing`
- Split `SigningProvider` into separate `Verifier` and `Signer` traits
- Split `SigningProviderExt` into `VerifierExt` and `SignerExt`
- Implement `Verifier` and `Signer` for `&T`, `Box<T>`, and `Arc<T>`
- Remove signing of proposal parts
- Remove `Signer::sign_bytes` and `Verifier::verify_signed_bytes`; every signing purpose is now a named trait method
- Promote `sign_validator_proof` and `verify_validator_proof` to required methods on `Signer` and `Verifier`; remove the `SignerExt` trait

### `sync`
- Validate sync response length against the requested range and credit partial
  responses through a new `SyncResult::PartialSuccess` variant, scaling the
  peer-score update by the `received / requested` ratio
- Reject sync responses with non-contiguous certificate heights
- Fix partial range request not being tracked in pending requests
- Preserve a sync_height rewind when a concurrent re-request to a different range succeeds, so the rewound range is picked up by the next request cycle instead of being silently abandoned
- Initial random (fixed) period adjustment in sync status ticker
- Refactor sync actor to notify consensus of sync responses
- Support batch retrieval of decided values
- Validate value request ranges before processing
- Introduce a new mode that sends a status update as soon as a new height is started rather than at a fixed interval ([#1452](https://github.com/circlefin/malachite/pull/1452))
  To enable this mode, set `status_update_interval = 0`.
- Queue sync responses for future heights in the Sync actor ([#1467](https://github.com/circlefin/malachite/pull/1467))
  Instead of buffering sync responses in the core-consensus input queue, sync responses are now buffered directly in the Sync actor.
  This prevents sync responses and consensus messages from contending over the input queue.

### `test`
- `ByzantineMiddleware` now lives under `malachitebft_test::byzantine` (previously under `malachitebft_engine_byzantine`); its constructor takes 5 args `(ignore_locks, force_precommit_nil, inner, self_address, seed)` and internally delegates to `Amnesia<TestContext>`

## 0.6.0

*November 19th, 2025*

- Remove `Effect::GetValidatorSet`, `AppMsg::GetValidatorSet` and `HostMsg::GetValidatorSet` ([#1189](https://github.com/circlefin/malachite/pull/1189))
- Introduce `malachitebft-signing` crate for exposing the `SigningProvider` and `SigningProviderExt` traits ([#1191](https://github.com/circlefin/malachite/pull/1191))
- Make `SigningProvider` trait methods fallible ([#1191](https://github.com/circlefin/malachite/pull/1191))
- Make `SigningProvider` trait methods async ([#1151](https://github.com/circlefin/malachite/issues/1151))
- Make GossipSub topic names configurable ([#849](https://github.com/circlefin/malachite/issues/849))
- Fix bug in WAL recovery logic where a corrupted entry would not be detected in some circumstances ([#1127](https://github.com/circlefin/malachite/pull/1127))
- Add facility for app to request a consensus state dump at any time ([#1176](https://github.com/circlefin/malachite/pull/1176))
- Make libp2p protocol names configurable ([#1161](https://github.com/circlefin/malachite/issues/1161))
- Fix mismatched height of WAL entries emitted when processing `StartHeight` input ([#1232](https://github.com/circlefin/malachite/issues/1232))

## 0.5.0

*July 31st, 2025*

- Update libp2p to v0.56.x ([#1124](https://github.com/circlefin/malachite/pull/1124))
- Rename `Effect::RebroadcastVote` to `Effect::RepublishVote` and `Effect::RebroadcastRoundCertificate` to `Effect::RepublishRoundCertificate` ([#1011](https://github.com/circlefin/malachite/issues/1011))
- Decouple `Host` messages from the `Consensus` actor ([#1109](https://github.com/circlefin/malachite/pull/1109))
- Fix a bug where values synced from other peers were assigned the current node's address instead of their proposer's address ([#1141](https://github.com/circlefin/malachite/pull/1141))
- Buffer sync values for heights higher than current height in consensus and replay when running consensus for those heights ([#1149](https://github.com/circlefin/malachite/pull/1149))
- Add value batching to sync messages ([#1070](https://github.com/circlefin/malachite/issues/1070))

## 0.4.0

*July 8th, 2025*

- Add parallel requests for the sync module ([#1092](https://github.com/circlefin/malachite/issues/1092))

## 0.3.1

*July 7th, 2025*

- Derive [Borsh](https://borsh.io) encoding for all core types, behind a `borsh` feature flag ([#1098](https://github.com/circlefin/malachite/pull/1098))
- Fixed a bug where the consensus engine would panic when the validator set is empty, now an error is properly emitted in the logs ([#1111](https://github.com/circlefin/malachite/pull/1111))
- When the sync module receives an invalid commit certificate from another peer, it will now drop the associated synced value altogether instead of passing it up to the application ([#1112](https://github.com/circlefin/malachite/pull/1112))

## 0.3.0

*June 17th, 2025*

- Removed the VoteSet synchronization protocol, as it is neither required nor sufficient for liveness ([#998](https://github.com/circlefin/malachite/issues/998))
- Reply to `GetValidatorSet` is now optional ([#990](https://github.com/circlefin/malachite/issues/990))
- Clarify and improve the application handling of multiple proposals for same height and round ([#833](https://github.com/circlefin/malachite/issues/833))
- Prune votes and polka certificates that are from lower rounds than node's `locked_round` ([#1019](https://github.com/circlefin/malachite/issues/1019))
- Add support for making progress in the presence of equivocating proposals ([#1018](https://github.com/circlefin/malachite/issues/1018))
- Take minimum available height into account when requesting values from peers ([#1074](https://github.com/circlefin/malachite/issues/1074))
- Add peer scoring system to the sync module with customizable scoring strategy ([#1072](https://github.com/circlefin/malachite/issues/1072))
  [See the corresponding PR](https://github.com/circlefin/malachite/pull/1071) for more details.

## 0.2.0

*April 16th, 2025*

- Add the capability to re-run consensus for a given height ([#893](https://github.com/circlefin/malachite/issues/893))
- Verify polka certificates ([#974](https://github.com/circlefin/malachite/issues/974))
- Use aggregated signatures in polka certificates ([#915](https://github.com/circlefin/malachite/issues/915))
- Improve verification of commit certificates ([#974](https://github.com/circlefin/malachite/issues/974))

## 0.1.0

*April 9th, 2025*

This is the first release of the Malachite consensus engine intended for general use.
This version introduces production-ready functionality with improved performance and reliability.

### Resources

- [The tutorial][tutorial] for building a simple application on top of Malachite using the high-level channel-based API.
- [ADR 003][adr-003] describes the architecture adopted in Malachite for handling the propagation of proposed values.
- [ADR 004][adr-004] describes the coroutine effect system used in Malachite.
  It is relevant if you are interested in building your own engine on top of the core consensus implementation of Malachite.


[tutorial]: ./docs/tutorials/channels.md
[adr-003]: ./docs/architecture/adr-003-values-propagation.md
[adr-004]: ./docs/architecture/adr-004-coroutine-effect-system.md

## 0.0.1

*December 19, 2024*

First open-source release of Malachite.
This initial version provides the foundational consensus implementation but is not recommended for production use.
