# Release Notes

## Unreleased

## 0.8.1

*September 24th, 2026*

### `discovery`
- With discovery off, keep a peer already counted as inbound from also being inserted into `outbound_peers` when a later dial reaches it. Close looks at outbound first, so the inbound slot used to leak and shrink usable inbound capacity

### `app`
- Reject `value_sync.parallel_requests = 0` when value sync is enabled, alongside the existing `batch_size = 0` rejection. A configuration that previously started with a degenerate zero-length request budget now fails at startup with an explicit error
- Reject `value_sync.request_timeout = 0` and `value_sync.max_request_size = 0` when value sync is enabled. Those values now reach the libp2p sync transport, so a zero timeout or request cap would fail every exchange instead of only the actor-side stall timer

### `app-channel`
- Carry the height's `VoteExtensionPolicy` on `AppMsg::ExtendVote`. Applications must reply `Some` when the policy is `Required`; an empty reply aborts the local precommit before WAL append and hangs WAL replay.
- Add `published_by` to `AppMsg::ReceivedProposalPart`, the publisher declared in the message. Group proposal part reassembly on it so the parts of one proposal come together, whichever peer relays each one, and keep charging resource limits to `from`
- Break the outbound network forwarder when a cast to the network actor fails, matching the engine recv-task. The application then sees `Err` on `Channels::network` instead of `Ok` for a dropped `PublishProposalPart` while the actor is already stopping

### `consensus`
- Apply the vote-extension policy to votes rebuilt from a round certificate. `RoundSignature` has no extension field, so a rebuilt non-nil precommit never carries one; under `Required` those votes are now dropped before WAL append and the driver, matching `on_vote` and WAL replay. Nil precommits and prevotes are unchanged. A Skip or PrecommitAny certificate whose threshold sits only in those dropped votes is a no-op; honest skip still happens through prevotes or nil precommits.
- Ask the application about vote extensions that arrive inside a synced commit certificate. Cryptographic verification still covers policy and signatures; the engine then asks the application about every attached extension in one `VerifyVoteExtensions` call, the same host message a live precommit uses for its single extension. If the application rejects one, the certificate is dropped and the peer is faulted.
- Carry the height's `VoteExtensionPolicy` on `Effect::ExtendVote` so the host can fulfill `Required` instead of treating the reply as optional. An empty reply under `Required` still errors before the local precommit is signed or WAL-appended.
- Skip verification and WAL appends for a round certificate whose votes are already in the vote keeper, including a resend and a quorum already collected over gossip. A certificate that still carries a missing or conflicting vote is verified as a bundle; only those new votes are WAL-appended.
- Reuse this node's own precommit vote extension recovered from the WAL instead of asking the application for a fresh one when replay causes the driver to vote again. A second extension for the same precommit can diverge from what peers already recorded.
- Retain at most three proposal-equivocation evidence pairs per validator and omit duplicate or unretained proposals from the WAL. Proposal identity is based on the signed message rather than the signature bytes, and WAL replay reconstructs every retained pair.
- Preserve the rebroadcast timeout when a `Skip` round certificate moves consensus into its round, so the node continues rebroadcasting its latest vote until the round advances
- Allow a proposed value from a full source-round bucket to complete an existing matching proposal at another round. If the value has a polka certificate instead, retain it at a deterministic certificate round. Both placements keep each bucket bounded and let re-proposals reach the consensus driver.
- Record proposal equivocation across distinct value ids when the second signed proposal arrives, instead of waiting for both proposals to be paired with their values. A proposer that signed two proposals for one `(height, round)` but streamed the parts for only one of them previously left no evidence at all
- Drop a conflicting vote once that validator's equivocation evidence is full, so a further distinct value is not verified or WAL-appended. The first conflicts still pass the already-seen gate so they can be recorded
- Cross-check every vote and proposal re-derived during WAL replay against what the node recorded before the crash: reuse the recorded message and its signature when they agree, sign only when the log holds no record for that height and round, and fail the replay when they disagree. Previously the re-derived message was signed afresh and published while the recorded one was dropped by the votekeeper's dedup, so a re-derivation that diverged reached peers with nothing but a warning. Vote extensions are excluded from the comparison, since the application supplies them and they need not be deterministic
- Exempt votes replayed from the WAL from the future-round lookahead bound. That bound exists to cap the work an untrusted peer can induce, and applying it to replay made the read path reject what the write path accepted: a round certificate can carry votes more than `MAX_FUTURE_ROUND_LOOKAHEAD` rounds ahead of the round replay starts from, so a node that skipped that far forward lost the round on restart — the polka, its own votes and any locked value with it. `State::is_replaying_wal` reports the window, tracked by the existing `index_wal_entries` / `reset_entries_index` pair
- Record the equivocation evidence carried by a polka or round certificate that the liveness handler skips. A certificate is identified by the polka it witnesses or the round it justifies, never by its signer set, so a certificate that duplicates one already held — or that no longer advances the round — could hold the only copy of a vote conflicting with one already recorded, and that misbehavior never reached the application at `Finalize`. A signature is verified only when recording it would retain a new pair, so a certificate that raises no conflict, repeats one already stored, or names a validator whose evidence is already capped still skips signature verification, WAL appends and driver inputs as before
- Apply the same `MAX_FUTURE_ROUND_LOOKAHEAD` bound to proposal messages that votes already use, so a far-future round cannot grow per-height proposal state, verification work, or the WAL. Proposed values are not bound this way: the WAL reconstructs them as consensus-origin and does not persist commit certificates, so a lookahead drop would discard a crash-recovered sync value

### `driver`
- Add `VoteKeeper::conflicting_vote`, `has_equivocation_evidence`, `can_record_equivocation` and `detect_equivocation` in `core-votekeeper`, so a layer that filters a vote before the normal vote path can still record its equivocation and can tell in advance whether doing so would retain anything. Add `RoundCertificate::votes` in `core-types`, matching the accessor its commit and polka counterparts already provide

### `discovery`
- Prune expired peers-request violations from the discovery rate limiter on any peer's request or disconnect, instead of only when the violating peer itself returns. A peer that earned a violation and never reconnected previously left its entry for the process lifetime

### `discovery`
- Clear `dial.done_on` on the last close of a connection, including one that ends before Identify accepts the peer. A pre-identify close previously left the record in place, so the one-second bootstrap tick skipped that peer until restart

### `engine`
- Load this node's own logged precommit extensions before WAL replay so `extend_vote` can reuse them instead of calling the application again
- Pass the height's `VoteExtensionPolicy` on `HostMsg::ExtendVote`. An explicit `None` from the host, or a signing failure, is logged with the height, round, and value and resumed as `None` so the driver raises `VoteExtensionRequired` rather than `UnexpectedResume`.
- Treat a dropped `ProcessSyncedValue` host reply as a local/transient error so the matching value-sync request slot is released instead of leaking until restart or prune
- Report a failed `Effect::RestreamProposal` host cast as a restream error instead of "sending decided value to host"
- Emit `PeerSubscribed` only the first time a `(peer, channel)` pair is seen, so a repeated Subscribe frame does not force another sync status broadcast. Bound the history-min-height host fetch for that broadcast with `request_timeout` so a silent host cannot stall the fetch indefinitely
- Treat an incomplete `Effect::VerifySignature` check as an invalid signature and keep the consensus actor running, matching the certificate verification paths. Previously a verifier `Err` was returned to `process!`, which resumed with `Continue` and failed the actor as `UnexpectedResume`
- Add `dropped_buffered_messages`, a counter over the messages the consensus actor discards instead of delivering, labelled by drop reason (`buffer_full` when the buffer was at capacity, `restart` when the height the message was buffered for was restarted). Both drops were previously invisible: the overflow emitted only a `warn!` whose result the caller discarded, and the restart path replaced the buffer with no log at all. The dropped message itself is logged, at `error!` when it carries a synced value — the sync actor keeps its range reserved until consensus advances past it and cannot recover on its own — and at `warn!` otherwise. `MessageBuffer::buffer` returns `Result<(), BufferFull<T>>` instead of `bool` so a call site can no longer ignore a drop
- Exempt `Msg::RestartHeight` from the consensus actor message buffer while not `Running`, matching `Msg::StartHeight`. A failed-commit restart that arrived outside `Running` was previously queued forever, leaving the node stuck at the failed height
- Carry the request id with each value from the sync actor through consensus and the host verdict, so `Msg::PeerFault` drops buffered values from the request that served the faulty value instead of whichever request covers the height when the fault is reported. This remains exact when two requests from the same peer buffer a value for the same height or when the host verdict arrives after the height is decided
- Carry the declared publisher to the host on `HostMsg::ReceivedProposalPart`, so a host can group proposal parts by publisher while keying per-peer resource limits on the delivering peer
- Run a dedicated `Msg::RetrySync` ticker at the `request_timeout` cadence, with a minimum interval of 1 second. The ticker sends `Input::TryRequestValues` in both status-update modes. `status_update_interval` controls only status broadcasts
- Verify a validator proof as soon as it arrives, even before `Phase::Running`, so a peer that connects during startup or recovery is classified before the buffer would have dropped the verdict on a height restart
- Ignore a `DecisionCommitted` acknowledgement captured before a `RestartHeight`, so a failed commit followed by `Next::Restart` cannot advance the value-sync tip past an uncommitted height. `Msg::DecisionCommitted` now carries the `CommitGeneration` stamped when `HostMsg::Decided` was sent
- Drop the inbound sync request slot when encoding an `OutgoingResponse` fails, and cancel the pending reply, so a host answer that cannot be encoded no longer leaks `inbound_requests`
- Index the node's own messages from the write-ahead log before replaying it, so consensus can cross-check each re-derived vote and proposal against its record instead of signing a fresh one. Embedders that drive `malachitebft-core-consensus` directly and never call `State::index_wal_entries` are unaffected

### `discovery`
- Clear the dial-history record in the form `dial_bootstrap_nodes` wrote it when a persistent peer is removed, including the `/p2p/` component. Stripping that component first left the record in place, so a later re-add was skipped every second until restart

### `discovery`
- Keep admitting a peer pinned by a bare `/p2p/<peer_id>` persistent-peer entry after its last connection closes. Disconnect still clears the runtime identify slot so an address-only bootstrap can be re-identified, but `persistent_peers_only` now also consults the configured `/p2p/` component, so a peer-only entry is not locked out.

### `network`
- Add optional application-payload limits for each consensus GossipSub topic through `p2p.pubsub_max_size_per_topic`. Malachite adds the signed-message overhead when it configures the wire limit. Unset topics continue to use the global `pubsub_max_size` limit. GossipSub rejects an oversized received message before signature verification, caching, delivery, and relay.
- Start the per-IP reconnect delay on every inbound close, not only when the last connection from that address has ended. A peer that keeps one connection open can no longer cycle the rest without waiting
- Send a validator proof on every new connection, not only the first, so a peer whose connection count diverged (a stale socket on our side, a new dial from theirs) still receives it. One copy is sent per established connection so `request_response`'s `request_id % n` routing cannot leave the new socket unserved. A second proof from the same peer in the same session is ignored instead of disconnecting. A mixed-version rollout can briefly disconnect: a node on this build will resend, and a peer still on the old anti-spam path treats that copy as spam until it upgrades too
- Drop a buffered validator proof when the peer's last connection closes, and do not store a verification verdict that arrives after that close. Previously a proof that completed after disconnect stayed in `pending_verified_proofs` until restart
- Restrict gossipsub inbound subscriptions to the consensus channel names (not Sync, which uses broadcast) so a peer cannot grow an unbounded per-peer topic set
- Drop inbound broadcast frames, subscribe events and unsubscribe events for channels this node never subscribed on that transport. Default gossipsub deployments only subscribe broadcast to Sync, so a peer can no longer deliver consensus, proposal-part or liveness frames on that path
- Pass the operator's value-sync `request_timeout`, `max_request_size`, and `parallel_requests` into the libp2p sync behaviour. Previously only `rpc_max_size` overrode the transport, so a 1s actor timeout still sat on a 10s request-response stream
- Refuse a pubsub send larger than `pubsub_max_size` in `pubsub::publish` instead of reporting a broadcast frame as published. Gossipsub already refused that frame later; the broadcast transport returned success and counted it as sent. Inbound scatter handlers still apply their own 4 MiB default
- Re-announce the ValueSync broadcast subscribe when another connection to the same peer opens, and when one connection closes while another remains. Scatter only sends `Subscribe` on a peer's first connection; a failed oneshot that leaves another connection up never retries, so that peer never hears this node and never sends it a status. Also skip status broadcasts while the local tip is still zero, and announce the first non-zero tip as soon as consensus starts above genesis, so Eager status mode (`status_update_interval == 0`) does not leave peers with a tip they cannot use until a decision
- Fix outbound repair silently staying short when Kademlia reported an empty candidate set as `Selection::Exactly` (all discovered peers already outbound or already requested). Selection is now classified by filtered count via `Selection::classify`, so an all-excluded result becomes `None` and repair starts a discovery extension. Last-connection cleanup now runs before that repair, so the extension does not peers-request the peer whose connection just closed
- Sanitize peer-supplied Identify monikers before they become Prometheus `peer_moniker` labels: keep ASCII alphanumeric characters plus `-`, `_`, and `.`, drop everything else, and consider at most 128 input characters, so a connected peer cannot inject metric lines into `/metrics`. Also refresh `explicit_peers` and `peer_mesh_membership` series when a connected peer's moniker changes, so the old series does not linger for the process lifetime
- Under `persistent_peers_only`, skip dialing peers learned from peer-exchange records, free the dial slot and close immediately when Identify rejects a non-persistent peer, so a peers request cannot fill all concurrent dial slots and leave discovery stuck in `Extending` with rejected peers still connected
- Free a Pending outbound slot if a Connect request is skipped because `done_on` is already set and nothing is in flight, so a still-connected refused peer cannot hold one of the configured outbound slots unused
- Grant inbound slots only after Identify completes, routing connect-request accepts through `promote_to_inbound` instead of inserting directly into the inbound set. A peer that received a slot before Identify finished was counted as inbound without appearing in `active_connections`, so it was invisible to eviction and could permanently fill the inbound budget, blocking validator and persistent peer promotion. Inbound eviction now prefers unidentified slot holders — peers in the inbound set with no `PeerInfo` — over the lowest-scoring identified non-priority peer
- Report both peer identities for inbound GossipSub messages. `Event::ConsensusMessage` carries the delivering neighbor (`propagation_source`) alongside the declared publisher (`message.source`), so resource limits can charge the connection while the parts of one proposal group are by publisher. `Event::LivenessMessage` carries only the delivering peer, since each liveness message stands alone. Messages with no declared source, previously dropped, are now attributed to the deliverer

### `discovery`
- Clear the peers-request "already asked" mark when the last connection to a peer closes, matching the existing connect-request cleanup, so a reconnecting peer can be asked for its peer list again
- Cap inbound peers-request records at `max_peers_per_response` before signature verification, matching the sibling processing loop, so one request cannot force more Ed25519 checks than the configured cap
- Route outbound discovery replies by the pending-request map that owns the `request_id`, not by response variant, so a wrong-type reply cannot leave a peers request stuck and stall extension

### `sync`
- Serve an inbound value-sync request longer than this node's own `batch_size` as a prefix no more than `batch_size` long, instead of answering it with an empty response. The requester scores a short but non-empty response as a partial success and requests the remainder on its next pass, so it no longer treats a peer with a smaller `batch_size` as faulty
- Raise `value_sync.batch_size` only once the peers that serve you run this version, since the change above is server-side: against older peers a larger `batch_size` still draws empty responses and stalls. Catch-up speed stays bounded by the peers' `batch_size`, so raising yours no longer stalls sync but does not speed it up either. Over-long requests also stop counting toward `value_inbound_request_failures{reason="invalid_range"}` — the new `value_inbound_requests_shortened` counter records them instead
- Reject an inbound value-sync request whose start height is below this node's `history_min_height`. A range that started below the prune floor was previously accepted, the host returned nothing, and the requester treated the empty answer as a peer fault
- Derive the libp2p `max_concurrent_streams` budget from `Config::effective_parallel_requests()`, so a `sync::Config` built with `parallel_requests = 0` outside `spawn_sync_actor` gets a non-zero stream budget instead of `0`
- Charge a peer fault to the request that delivered the value rather than to whichever request covers the height when the fault is reported. Those differ once a retry has taken the range over, and the old lookup then dropped the replacement's buffered values, barred it from serving the range again, and re-issued a request it was already answering. `Input::PeerFault` carries the originating request id
- Bar only the peer a re-request's caller names, instead of also barring the peer recorded on the pending entry whenever the two differ. The two can only differ if our own record of which peer a request was sent to is wrong — a response cannot be attributed to a peer the request never went to, since that attribution comes from the connection it arrived on. Barring the recorded peer therefore answered a local bookkeeping error by removing a peer that had done nothing observable, shrinking the eligible set for the range
- Add `Input::TryRequestValues` to start a request pass from cached peer state without a status broadcast. The trigger does not require a new peer status or consensus progress. It retries after the request frontier rewinds and no request remains pending
- Key the outbound request latency metric by request id rather than the request range's start height, and release the entry wherever a request is retired. Abandoning a request without a replacement previously leaked its entry permanently, since a height consensus has validated is never requested again

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
- Release a pending inbound sync request as soon as the connection carrying it closes, instead of waiting out the inbound stall timer. The request's host call is aborted, its per-peer in-flight slot and semaphore permits are released, and the network layer drops its response channel. Requests lost this way are recorded under a new `connection_closed` reason on `value_inbound_request_failures`, distinct from `host_stall_timeout`, which continues to mean the local host was slow. A peer resetting an individual substream while remaining connected stays undetectable at the pinned libp2p and yamux versions, and is still covered by the stall timer
- Remove the WAL-replay delay: a restarting validator now replays its WAL immediately instead of first waiting for value sync to fetch a certificate for the crash height. This deletes the `WaitingForSync` phase, its timer, its pending-entry buffer and the sync-message buffer bypass, along with the `consensus.wal_replay_delay` config option. Two consequences of the removed path are gone with it: the driver reset that preceded the deferred replay could report a round going backwards to the application, and the same path could make a validator equivocate against its own earlier votes. Recovery from a chain halt is also no longer delayed by the configured amount, since in that situation no peer can serve the certificate the delay was waiting for

### `driver`
- Add `IntoIterator` impls and `len()` to `EvidenceMap` in `core-driver` and `core-votekeeper`

### `network`
- Support peer-only multiaddrs (`/p2p/<peer_id>`) in `persistent_peers`: entries without a transport component are used for inbound identity filtering and are never dialed
- Make GossipSub topic / broadcast channel names configurable via `P2pConfig.channel_names`; channel names are validated for non-emptiness and uniqueness before the network actor is spawned
- Emit `Event::SyncInboundRequestFailed` when libp2p reports that the connection carrying a pending inbound sync request closed, and drop the request's response channel on `CtrlMsg::SyncCancelReply`. A custom network layer that emits neither keeps the previous behaviour, where the engine's inbound stall timer releases the request

### `signing`
- Bind vote-extension signatures to their precommit scope `(height, round, value_id, validator_address)` so an extension blob cannot be relayed across heights, rounds, values, or validators
- Add `ExtendedCommitCertificate<Ctx>`, a self-verifiable bundle of per-validator precommit signatures and their optional vote extensions, with constructors that rebuild it from raw votes (`from_votes`) or from the host API's parallel `(CommitCertificate, VoteExtensions)` pair (`from_commit_certificate_and_extensions`). Verify the whole bundle in one pass via `VerifierExt::verify_extended_commit_certificate`
- Sync now carries vote extensions: `ValueResponse`, `RawDecidedValue`, `RawDecidedBlock`, and the on-disk `DecidedValue` all hold `ExtendedCommitCertificate` so a node catching up via sync can propose the next height when the application uses extensions for load-bearing data. Applications choose per height whether extensions must be absent or present via `HeightParams::with_vote_extension_policy(VoteExtensionPolicy::{Disabled, Required})`. Fixes the proposer-after-sync corner case.

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
