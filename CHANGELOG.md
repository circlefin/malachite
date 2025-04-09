# Changelog

## 0.1.0

*April 9, 2025*

## 📖 Release notes
See [`RELEASE_NOTES.md`](./RELEASE_NOTES.md#0.1.0) for the release notes.

### ⚠️ Breaking changes
See [`BREAKING_CHANGES.md`](./BREAKING_CHANGES.md#0.1.0) for the list of breaking changes.

### 🧪 Specifications

- *(spec)* New height logic / fast track ([#591](https://github.com/informalsystems/malachite/pull/591))
- *(spec)* Fix typo in specs
- *(spec)* Improvements and fixes in consensus overview ([#738](https://github.com/informalsystems/malachite/pull/738))
- *(spec)* Requirements for the proposer selection algorithm ([#739](https://github.com/informalsystems/malachite/pull/739))
- *(spec)* Requirements on valid(v) ([#748](https://github.com/informalsystems/malachite/pull/748))
- *(spec)* Discussion of consensus `getValue()` function ([#760](https://github.com/informalsystems/malachite/pull/760))
- *(spec)* Consensus network requirements and gossip property ([#764](https://github.com/informalsystems/malachite/pull/764))
- *(spec)* Consensus timeouts and synchrony assumptions ([#765](https://github.com/informalsystems/malachite/pull/765))
- *(spec)* Argue that Tendermint is a safe and live consensus algorithm ([#777](https://github.com/informalsystems/malachite/pull/777))
- *(spec/test)* Model-based tests for part streaming ([#814](https://github.com/informalsystems/malachite/pull/814))
- *(spec/consensus)* Small fixes to overview.md file.  ([#873](https://github.com/informalsystems/malachite/pull/873))

### 🚀 Features

- *(code/app)* Add persistence to the example app ([#746](https://github.com/informalsystems/malachite/pull/746))
- *(code/consensus)* Add support for full nodes ([#750](https://github.com/informalsystems/malachite/pull/750))
- *(code)* Feature-gate metrics in `core-consensus` crate ([#762](https://github.com/informalsystems/malachite/pull/762))
- *(code/example)* Add metrics to the example app's store ([#636](https://github.com/informalsystems/malachite/pull/636))
- *(code)* Make `peer` crate `no_std` compatible ([#786](https://github.com/informalsystems/malachite/pull/786))
- *(code)* Refactor parameters of `spawn_mempool_actor` in starknet host
- *(code)* Wrap `Vec<Validator>` into `Arc` in `ValidatorSet` struct ([#795](https://github.com/informalsystems/malachite/pull/795))
- *(code)* Check proposal validity in the example app ([#793](https://github.com/informalsystems/malachite/pull/793))
- *(code)* Add extensions to the test app's `Value` type ([#799](https://github.com/informalsystems/malachite/pull/799))
- *(code/app/starknet)* Include existing transactions when reaping the mempool not just the ones generated on the fly ([#796](https://github.com/informalsystems/malachite/pull/796))
- *(code)* Vote extensions v2 ([#775](https://github.com/informalsystems/malachite/pull/775))
- *(code/app/starknet)* Adapt Starknet app to latest P2P protos ([#819](https://github.com/informalsystems/malachite/pull/819))
- *(code/test)* Add channel-based test app for integration tests ([#747](https://github.com/informalsystems/malachite/pull/747))
- *(code)* Add Makefile to fix lints and test in local ([#751](https://github.com/informalsystems/malachite/pull/751))
- *(code/vote_sync)* Rebroadcast votes at prevote or precommit timeout interval ([#828](https://github.com/informalsystems/malachite/pull/828))
- *(code/test)* Generalize the test framework to make it work with any app ([#836](https://github.com/informalsystems/malachite/pull/836))
- *(code/engine)* Buffer consensus messages during startup and recovery ([#860](https://github.com/informalsystems/malachite/pull/860))
- *(code/example)* Simulate processing time in example app
- *(code/core-consensus)* Add `vote_sync_mode` option to consensus parameters ([#870](https://github.com/informalsystems/malachite/pull/870))
- *(code)* Allow applications to define their own configuration type ([#872](https://github.com/informalsystems/malachite/pull/872))
- *(code/wal)* Add facilities for dumping the WAL entries ([#903](https://github.com/informalsystems/malachite/pull/903))
- *(code)* Add polka certificates to VoteSync response ([#859](https://github.com/informalsystems/malachite/pull/859))
- *(code/network)* Only enable the network behaviors which are actually required ([#928](https://github.com/informalsystems/malachite/pull/928))
- *(code/starknet)* Add mempool load generator to the Starknet test app ([#821](https://github.com/informalsystems/malachite/pull/821))
- *(code/app)* Change the log level dynamically when consensus enters round > 0 ([#913](https://github.com/informalsystems/malachite/pull/913))
- *(code/test)* Middleware for testing ([#948](https://github.com/informalsystems/malachite/pull/948))
- *(code/engine)* Store proposed values in WAL ([#896](https://github.com/informalsystems/malachite/pull/896))

### 🐛 Bug Fixes

- *(code/core-state-machine)* Remove unused `PrecommitValue` from `Input` ([#797](https://github.com/informalsystems/malachite/pull/797))
- *(code/core-consensus)* Break out of loop instead of returning in `process!` macro ([#816](https://github.com/informalsystems/malachite/pull/816))
- *(code/core-consensus)* Verify vote extension on precommits ([#820](https://github.com/informalsystems/malachite/pull/820))
- *(code/app)* Fix bug in test app and example app where proposal parts were not processed in order of sequence number ([#851](https://github.com/informalsystems/malachite/pull/851))
- *(code/core-consensus)* Only cancel propose timeout on a step change ([#862](https://github.com/informalsystems/malachite/pull/862))
- *(code/core-consensus)* Prevent nodes from broadcasting equivocating votes ([#864](https://github.com/informalsystems/malachite/pull/864))
- *(code/example)* Fix path to example apps store db
- *(code/network)* Notify new subscribers of listen addresses and connected peers ([#876](https://github.com/informalsystems/malachite/pull/876))
- *(code/engine)* Use custom version of `OutputPort` with configurable capacity ([#878](https://github.com/informalsystems/malachite/pull/878))
- *(code/discovery)* Simplified disabled discovery + reduced logs verbosity ([#892](https://github.com/informalsystems/malachite/pull/892))
- *(code/engine)* Ensure the height in the tracing span matches the height we are about to start ([#890](https://github.com/informalsystems/malachite/pull/890))
- *(code)* Fix various sync-related bugs seen in testnet ([#910](https://github.com/informalsystems/malachite/pull/910))
- *(code)* Add driver tests for l30 and l32 of the algorithm when the old polka vote arrives last ([#916](https://github.com/informalsystems/malachite/pull/916))
- *(code/core-consensus)* Only persist votes in the WAL if they have not yet been seen ([#902](https://github.com/informalsystems/malachite/pull/902))
- *(code)* Full nodes send empty commit certificates when deciding ([#920](https://github.com/informalsystems/malachite/pull/920))
- *(code)* Remove transport argument in p2p config ([#938](https://github.com/informalsystems/malachite/pull/938))
- *(code)* Handle multiple commits for same height ([#921](https://github.com/informalsystems/malachite/pull/921))
- *(code/network)* Support both `quic` and `quic-v1` protocol specifiers in multiaddr ([#958](https://github.com/informalsystems/malachite/pull/958))

### 📄 Documentation

- *(docs)* ADR 003 - Propagation of Proposed Values ([#884](https://github.com/informalsystems/malachite/pull/884))
- *(docs)* ADR 004 - Coroutine-Based Effect System for Consensus ([#931](https://github.com/informalsystems/malachite/pull/931))

## 0.0.1

*December 19, 2024*

## 📖 Release notes
See [`RELEASE_NOTES.md`](./RELEASE_NOTES.md#0.0.1) for the release notes.

### ⚠️ Breaking changes
See [`BREAKING_CHANGES.md`](./BREAKING_CHANGES.md#0.0.1) for the list of breaking changes.

### 🧪 Specifications

- *(spec/votekeeper)* Fix the VoteKeeper spec to account for skip threshold from higher round ([#74](https://github.com/informalsystems/malachite/pull/74))
- *(spec)* Clean up, addresses all points in issue 151 ([#154](https://github.com/informalsystems/malachite/pull/154))
- *(spec)* Asynchronous flow for `GetValue` ([#159](https://github.com/informalsystems/malachite/pull/159))
- *(spec)* WIP: Draft of Quint reset protocol ([#174](https://github.com/informalsystems/malachite/pull/174))
- *(spec)* Reset protocol English specification ([#171](https://github.com/informalsystems/malachite/pull/171))
- *(spec)* Message handling section of english spec revisited ([#162](https://github.com/informalsystems/malachite/pull/162))
- *(spec)* Proofs scheduling protocol, English specification ([#241](https://github.com/informalsystems/malachite/pull/241))
- *(spec)* Proofs spec ([#198](https://github.com/informalsystems/malachite/pull/198))
- *(spec)* Add properties, and write report on the findings ([#189](https://github.com/informalsystems/malachite/pull/189))
- *(spec)* A problematic run ([#101](https://github.com/informalsystems/malachite/pull/101))
- *(spec)* Added equivocation recording ([#292](https://github.com/informalsystems/malachite/pull/292))
- *(spec)* Proofs scheduling, critical scenario ([#298](https://github.com/informalsystems/malachite/pull/298))
- *(spec)* Overriding an existing set of Precommits in the vote keeper with a new Commit. ([#293](https://github.com/informalsystems/malachite/pull/293))
- *(spec)* Maintain weights in vote keeper ([#321](https://github.com/informalsystems/malachite/pull/321))
- *(spec)* Handle equivocation liveness issues by introducing certificates ([#364](https://github.com/informalsystems/malachite/pull/364))
- *(spec)* Description of misbehavior and what can be handled now ([#388](https://github.com/informalsystems/malachite/pull/388))
- *(spec)* Reorganization, consensus general overview ([#386](https://github.com/informalsystems/malachite/pull/386))
- *(spec)* Accountable Tendermint ([#404](https://github.com/informalsystems/malachite/pull/404))
- *(spec)* Consensus spec, from paper, general overview ([#389](https://github.com/informalsystems/malachite/pull/389))
- *(spec/consensus)* Consensus `overview.md` document ([#511](https://github.com/informalsystems/malachite/pull/511))
- *(spec)* Blocksync ([#462](https://github.com/informalsystems/malachite/pull/462))
- *(spec)* English specification for the sync protocol ([#548](https://github.com/informalsystems/malachite/pull/548))
- *(spec)* Refactor blocksync into two statemachines ([#564](https://github.com/informalsystems/malachite/pull/564))
- *(spec)* Add timeout to Blocksync ([#568](https://github.com/informalsystems/malachite/pull/568))
- *(spec)* Blocksync request/response for block and commit ([#575](https://github.com/informalsystems/malachite/pull/575))
- *(spec)* Mocked consensus for blocksync patched ([#590](https://github.com/informalsystems/malachite/pull/590))
- *(spec/quint)* Blocksync files and modules nomenclature ([#592](https://github.com/informalsystems/malachite/pull/592))
- *(spec)* Blocksync witnesses and invariants ([#594](https://github.com/informalsystems/malachite/pull/594))
- *(spec)* Rewrite of English blocksync spec ([#589](https://github.com/informalsystems/malachite/pull/589))
- *(spec)* New `init` action to find witnesses in blocksync with consensus model ([#601](https://github.com/informalsystems/malachite/pull/601))
- *(spec)* Quint tests for blocksync protocol ([#615](https://github.com/informalsystems/malachite/pull/615))
- *(spec)* New structure for the `specs/` directory ([#602](https://github.com/informalsystems/malachite/pull/602))
- *(spec)* Rename BlockSync to ValueSync ([#698](https://github.com/informalsystems/malachite/pull/698))

### 🚀 Features

- *(code)* Propagate height to `GetValue` recipient and split `GetValueAndScheduleTimeout` into two outputs ([#149](https://github.com/informalsystems/malachite/pull/149))
- *(code)* Actor-based node implementation ([#167](https://github.com/informalsystems/malachite/pull/167))
- *(code)* Remove dependency on `ProposerSelector` from consensus ([#179](https://github.com/informalsystems/malachite/pull/179))
- *(code)* Add a `CAL` actor with `GetValue`, `GetProposer` and `GetValidatorSet` ([#180](https://github.com/informalsystems/malachite/pull/180))
- *(code)* Synchronized start of consensus when all validators/nodes connected ([#190](https://github.com/informalsystems/malachite/pull/190))
- *(code)* Initial configuration options ([#183](https://github.com/informalsystems/malachite/pull/183))
- *(code)* Test implementations for mempool, tx batch streaming and value builder ([#191](https://github.com/informalsystems/malachite/pull/191))
- *(code)* Add `ValueBuilder` test params to the config ([#206](https://github.com/informalsystems/malachite/pull/206))
- *(code)* Increase timeouts by the configured delta when they elapse ([#207](https://github.com/informalsystems/malachite/pull/207))
- *(code)* Add `max_block_size` config option ([#210](https://github.com/informalsystems/malachite/pull/210))
- *(code)* Starknet host integration ([#236](https://github.com/informalsystems/malachite/pull/236))
- *(code)* Extract consensus actor logic into a library devoid of side-effects ([#274](https://github.com/informalsystems/malachite/pull/274))
- *(code)* Add equivocation detection to the `VoteKeeper` ([#311](https://github.com/informalsystems/malachite/pull/311))
- *(code)* Integrate with `starknet-p2p-specs` protos ([#286](https://github.com/informalsystems/malachite/pull/286))
- *(code)* Remove built-in protos in order to better integrate with Starknet protos ([#326](https://github.com/informalsystems/malachite/pull/326))
- *(code)* Use Starknet hashing and signing schemes ([#295](https://github.com/informalsystems/malachite/pull/295))
- *(code)* Implement proposal parts streaming ([#341](https://github.com/informalsystems/malachite/pull/341))
- *(code)* Use the `jemalloc` memory allocator on Linux ([#372](https://github.com/informalsystems/malachite/pull/372))
- *(code)* Add support for `libp2p-broadcast` as an alternative to GossipSub ([#354](https://github.com/informalsystems/malachite/pull/354))
- *(code)* Add support for TCP transport with libp2p ([#382](https://github.com/informalsystems/malachite/pull/382))
- *(code)* Add test for pubsub protocols with different message sizes ([#360](https://github.com/informalsystems/malachite/pull/360))
- *(code)* Start next height on demand from the host instead of implicitly ([#385](https://github.com/informalsystems/malachite/pull/385))
- *(code)* Keep signed votes and proposals in their respective keepers ([#424](https://github.com/informalsystems/malachite/pull/424))
- *(code)* Add consensus traces ([#351](https://github.com/informalsystems/malachite/pull/351))
- *(code/metrics)* Add `malachite_consensus_consensus_time` metric for tracking consensus time without proposal building time ([#447](https://github.com/informalsystems/malachite/pull/447))
- *(code/discovery)* Basic discovery protocol ([#402](https://github.com/informalsystems/malachite/pull/402))
- *(code)* Add `select_proposer` method to `Context` trait ([#471](https://github.com/informalsystems/malachite/pull/471))
- *(code)* Vote Extensions ([#456](https://github.com/informalsystems/malachite/pull/456))
- *(code)* BlockSync MVP ([#440](https://github.com/informalsystems/malachite/pull/440))
- *(code/blocksync)* Add support for large blocks to BlockSync ([#508](https://github.com/informalsystems/malachite/pull/508))
- *(code)* Add persistence to the block store ([#514](https://github.com/informalsystems/malachite/pull/514))
- *(code)* Restream values for which we have seen a polka in a previous round ([#506](https://github.com/informalsystems/malachite/pull/506))
- *(code)* Add consensus modes to support implicit and explicit only proposals ([#522](https://github.com/informalsystems/malachite/pull/522))
- *(code)* Build Rust documentation on CI and push it to GitHub pages ([#538](https://github.com/informalsystems/malachite/pull/538))
- *(code)* Verify commit certificate ([#532](https://github.com/informalsystems/malachite/pull/532))
- *(code)* Add signature to vote extensions ([#537](https://github.com/informalsystems/malachite/pull/537))
- *(code/blocksync)* On invalid commit certificate, request block from a different peer ([#541](https://github.com/informalsystems/malachite/pull/541))
- *(code/metrics)* Add `moniker` label with node's moniker to all metrics ([#539](https://github.com/informalsystems/malachite/pull/539))
- *(code)* Start consensus at `H+1` where `H` is the height of the latest committed block ([#587](https://github.com/informalsystems/malachite/pull/587))
- *(code/app)* Introduce channel-based interface for building applications ([#603](https://github.com/informalsystems/malachite/pull/603))
- *(code/wal)* Add Write-Ahead Log implementation ([#608](https://github.com/informalsystems/malachite/pull/608))
- *(code/wal)* Persist and replay timeouts and consensus messages using the WAL ([#613](https://github.com/informalsystems/malachite/pull/613))
- *(code)* Vote synchronization when consensus is stuck in Prevote or Precommit ([#617](https://github.com/informalsystems/malachite/pull/617))
- *(code/wal)* Replay proposed values from the store instead of from the WAL ([#633](https://github.com/informalsystems/malachite/pull/633))
- *(code/discovery)* Kademlia-based discovery ([#525](https://github.com/informalsystems/malachite/pull/525))

### 🐛 Bug Fixes

- *(code)* Create overflow protection in vote calculations (removed FIXMEs) ([#193](https://github.com/informalsystems/malachite/pull/193))
- *(code)* Do not cancel prevote/precommit timeout when threshold is reached ([#208](https://github.com/informalsystems/malachite/pull/208))
- *(code)* Fix memory leak in `ValueBuilder` ([#230](https://github.com/informalsystems/malachite/pull/230))
- *(code)* Use more reasonable `history_gossip` and `history_length` config params for GossipSub ([#261](https://github.com/informalsystems/malachite/pull/261))
- *(code)* Update to `libp2p-tls` v0.4.1+ ([#288](https://github.com/informalsystems/malachite/pull/288))
- *(code/consensus)* Various fixes for consensus ([#330](https://github.com/informalsystems/malachite/pull/330))
- *(code/mempool)* Fix bug where mempool would not gossip any txes when `txs_per_part < gossip_batch_size` ([#349](https://github.com/informalsystems/malachite/pull/349))
- *(code/driver)* Fix duplicated work by not storing the proposal prior to passing it as input to the driver ([#475](https://github.com/informalsystems/malachite/pull/475))
- *(code/discovery)* Improve discovery performance ([#483](https://github.com/informalsystems/malachite/pull/483))
- *(code/discovery)* Changed logging condition ([#512](https://github.com/informalsystems/malachite/pull/512))
- *(code)* Fix restream bug surfaced by `ProposalFin` signature ([#540](https://github.com/informalsystems/malachite/pull/540))
- *(code/blocksync)* Broadcast BlockSync status instead of gossiping it ([#611](https://github.com/informalsystems/malachite/pull/611))
- *(code)* Fix build on `main` ([#631](https://github.com/informalsystems/malachite/pull/631))
- *(code/network)* Fix flaky tests and outgoing port re-use issue ([#647](https://github.com/informalsystems/malachite/pull/647))
- *(code/wal)* Fix WAL tests ([#653](https://github.com/informalsystems/malachite/pull/653))
- *(code)* Fix logging with new crate names ([#700](https://github.com/informalsystems/malachite/pull/700))
- *(code)* Fix serialization of `ephemeral_connection_timeout` config option ([#708](https://github.com/informalsystems/malachite/pull/708))
- *(code)* Remove unused dependency on the test mempool

### 📄 Documentation

- *(docs)* Fix references in spec doc ([#455](https://github.com/informalsystems/malachite/pull/455))
- *(docs)* Updates to `testing/local.md` docs ([#454](https://github.com/informalsystems/malachite/pull/454))
- *(docs)* Evidence blog post ([#544](https://github.com/informalsystems/malachite/pull/544))

<!-- generated by git-cliff -->
