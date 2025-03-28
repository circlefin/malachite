# Write an in-process Malachite application

<!-- TOC start (generated with https://github.com/derlin/bitdowntoc) -->

## Table of contents
- [Introduction](#introduction)
- [Naming](#naming)
- [Prerequisites](#prerequisites)
- [Concepts](#concepts)
   * [The `malachitebft-app-channel` crate](#the-malachitebft-app-channel-crate)
   * [The `Context` trait](#the-context-trait)
   * [Consensus types](#consensus-types)
   * [The `Codec` trait](#the-codec-trait)
   * [The `Node` trait](#the-node-trait)
   * [Messages from consensus to the application](#messages-from-consensus-to-the-application)
   * [Application state](#application-state)
- [Putting it all together](#putting-it-all-together)
   * [Create a new Rust project](#create-a-new-rust-project)
   * [Application state](#application-state-1)
   * [The consensus dialog](#the-consensus-dialog)
   * [Handle application messages](#handle-application-messages)
   * [Node](#node)
   * [Logging](#logging)
   * [Command-line interface](#command-line-interface)
- [Run a local testnet](#run-a-local-testnet)

<!-- TOC end -->


## Introduction
In this tutorial we will build an example validator application using the Malachite libraries. The focus is
integration with the Malachite consensus engine using [Tokio](https://tokio.rs) channels.

## Naming
While Malachite is comprised of several crates whose name start `informalsystems-malachitebft-`,
in this document we will use a shortened prefix `malachitebft-`, thanks to Cargo's ability
to expose a dependency under a different name than the one derived from its crate name.
More about this in the [Putting it all together](#putting-it-all-together) section.

## Prerequisites
The tutorial assumes basic knowledge of asynchronous programming in Rust using the Tokio library.
The beginner Rust knowledge is essential, the asynchronous programming knowledge is recommended.

The tutorial assumes basic distributed systems knowledge, for example: what is a validator, what is a Proof-of-Stake consensus engine.

Knowledge of [CometBFT](https://cometbft.com) or other Byzantine-fault tolerant consensus engines may help with
understanding the consensus engine concepts, however it is not required.

## Concepts
Before going any further, the reader might want to go over the [`ARCHITECTURE.md`](/ARCHITECTURE.md) document
for background information on Malachite and the ideas behind its architecture.

We can now get familiar with the concepts pertaining to building an application for Malachite.

### The `malachitebft-app-channel` crate
An example application will require only a few of the Malachite crates. The `malachitebft-app-channel` crate has all the
necessary components for building an application that interacts with the consensus engine through Tokio channels.
The crate also re-exports the necessary types and traits from the `malachitebft-app` crate under
`malachitebft_app_channel::app` for easier consumption.

### The `Context` trait

Because Malachite is a generic implementation of BFT consensus engine, it endeavours to make as few assumptions
as possible about the concrete data structures it uses, and leaves their implementation up to the application.

In order to do that, the `Context` trait provides an abstraction over the various data types used in the engine.
It is defined in `malachitebft_app_channel::app::types::core::Context` and an example implementation can be seen at
`malachitebft_test::Context`.

```rust
pub trait Context
where
    Self: Sized + Clone + Send + Sync + 'static,
{
    /// The type of address of a validator.
    type Address: Address;

    /// The type of the height of a block.
    type Height: Height;

    /// The type of proposal part
    type ProposalPart: ProposalPart<Self>;

    /// The interface provided by the proposal type.
    type Proposal: Proposal<Self>;

    /// The interface provided by the validator type.
    type Validator: Validator<Self>;

    /// The interface provided by the validator set type.
    type ValidatorSet: ValidatorSet<Self>;

    /// The `Value` type denotes the value `v` carried by the `Proposal`
    /// consensus message that is gossiped to other nodes by the proposer.
    type Value: Value;

    /// The type of votes that can be cast.
    type Vote: Vote<Self>;

    /// The type of vote extensions.
    type Extension: Extension;

    /// The signing scheme used to sign consensus messages.
    type SigningScheme: SigningScheme;

    // ...
}
```

The application is expected to instantiate this `Context` trait with concrete types for the abstract type definitions above.
Each of these concrete will need to implement the corresponding trait, which can be found on the right-hand side of the
type definition.

The `Context` also defines a few abstract methods which need to be implemented:

```rust
pub trait Context {
    // ...

    /// Select a proposer in the validator set for the given height and round.
    fn select_proposer<'a>(
        &self,
        validator_set: &'a Self::ValidatorSet,
        height: Self::Height,
        round: Round,
    ) -> &'a Self::Validator;

    /// Build a new proposal for the given value at the given height, round and POL round.
    fn new_proposal(
        height: Self::Height,
        round: Round,
        value: Self::Value,
        pol_round: Round,
        address: Self::Address,
    ) -> Self::Proposal;

    /// Build a new prevote vote by the validator with the given address,
    /// for the value identified by the given value id, at the given round.
    fn new_prevote(
        height: Self::Height,
        round: Round,
        value_id: NilOrVal<ValueId<Self>>,
        address: Self::Address,
    ) -> Self::Vote;

    /// Build a new precommit vote by the validator with the given address,
    /// for the value identified by the given value id, at the given round.
    fn new_precommit(
        height: Self::Height,
        round: Round,
        value_id: NilOrVal<ValueId<Self>>,
        address: Self::Address,
    ) -> Self::Vote;
}
```

### Consensus types
The basic consensus types, like the `Height` of a network, the `Address` of a wallet, the description of a `Validator`, or a set of
validators (`ValidatorSet`) are defined as traits in `malachitebft_app_channel::app::types`.

For example, the `Height` trait requires these three methods to be implemented:
* `increment_by`
* `decrement_by`
* `as_u64`

Additional methods, like `increment` or `decrement` have default implementations that can be overwritten.

Example implementation of the `Height` trait:
```rust
/// A blockchain height
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Height(u64);

impl malachitebft_app_channel::app::types::core::Height for Height {
    fn increment_by(&self, n: u64) -> Self {
        Self(self.0 + n)
    }

    fn decrement_by(&self, n: u64) -> Option<Self> {
        Some(Self(self.0.saturating_sub(n)))
    }

    fn as_u64(&self) -> u64 {
        self.0
    }
}
```
This implementation is an excerpt from a struct implemented in the `malachitebft_test` crate.

In this tutorial, for the sake of simplicity, we will use these pre-defined types from the `malachitebft_test` crate
instead of defining our own.

Note the `malachitebft_test::Value` implementation:
```rust
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Value(u64);

impl malachitebft_app_channel::app::types::core::Value for Value {
    type Id = ValueId;

    fn id(&self) -> ValueId {
        self.id()
    }
}
```

The test implementation defines a very simple type of values for consensus to decide on.
We will use this for now, but a real application would likely use something more akin
to a [*block*](https://github.com/tendermint/spec/blob/8dd2ed4c6fe12459edeb9b783bdaaaeb590ec15c/spec/core/data_structures.md#block),
with a proper header and a list of transactions included in that block, etc.


### The `Codec` trait
Nodes on the network need to communicate with each other. Implementing the `encode` and `decode` methods of the
`malachitebft_codec::Codec` trait defines how messages are encoded and decoded when sent over the wire.
Typically, Protobuf is a very common choice for encoding/decoding messages but to keep modularity flexible, there is no default implementation.
The `malachitebft_test::codec::proto::ProtobufCodec` implementation can be used as an example, and for testing.

The following types defined by the `Context`, need to have a `Codec` implementation,
where `Ctx` is the type of the concrete `Context` used by the application.
* `Ctx::ProposalPart`

The following types are also sent over the wire and need a `Codec` implementation,
where `Ctx` is the type of the concrete `Context` used by the application.
* `malachitebft_app_channel::app::types::SignedConsensusMsg<Ctx>`
* `malachitebft_app_channel::app::types::streaming::StreamMessage<Ctx::ProposalPart>`

Moreover, some messages are used during synchronization among different nodes.
These messages also need to be encoded and decoded when sent over the wire:
* `malachitebft_app_channel::app::types::sync::Status`
* `malachitebft_app_channel::app::types::sync::Request`
* `malachitebft_app_channel::app::types::sync::Response`

### The `Node` trait
The `malachitebft_app_channel::app::Node` trait allows the application to define how to load its configuration, genesis file and private key
from the filesystem or some other medium.

In order to generate a configuration file with the `init` command, the `Node` trait also defines
how to generate a signing key, encode and decode it from the underlying storage medium, and extracting a public key and
a wallet address from it. It is also responsible for loading the genesis file from the storage or generating one for testing purposes.

The `Node::run()` method is the entry point for the application where the initial configuration is loaded and parsed and
the Malachite actors (consensus engine, network, etc.) are started.


### Messages from consensus to the application
While running, the consensus engine will send messages to the application, describing steps taken by consensus,
or requesting an action to be performed by the application.
In cases when a `reply_to` field is present, the application will need to send a response back to the engine.
In any case, the message received can be used to change the internal state of the application:
assemble a value to propose, break down a proposal into parts and broadcast them over the network, etc.

**Note:**
While the internal implementation of Malachite is based on the actor model, the `malachitebft_app_channel`
crate provides a layer over it that uses Tokio channels instead of actors for communicating with the engine,
so that the application does not have to buy into the actor model.

The messages that can be received and have to be handled by the application are defined by `malachitebft_app_channel::AppMsg` type.
A brief description of each message can be found below:

| Message                | Description                                                                                                                                                                                                                                                                                                                                                                                                                                |
|------------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `ConsensusReady`       | Notifies the application that consensus is ready. The application MAY reply with a message to instruct consensus to start at a given height.                                                                                                                                                                                                                                                                                               |
| `StartedRound`         | Notifies the application that a new consensus round has begun.                                                                                                                                                                                                                                                                                                                                                                             |
| `GetValue`             | Requests the application to build a value for consensus to run on. The application MUST reply to this message with the requested value within the specified timeout duration.                                                                                                                                                                                                                                                              |
| `ExtendVote`  | Allows the application to extend the pre-commit vote with arbitrary data. When consensus is preparing to send a pre-commit vote, it first calls `ExtendVote`. The application then returns a blob of data called a vote extension. This data is opaque to the consensus algorithm but can contain application-specific information. The proposer of the next block will receive all vote extensions along with the commit certificate.                                                                                                                                                                                                                                                                                   |
| `VerifyVoteExtension`  | Requests the application to verify a vote extension. If the vote extension is deemed invalid, the vote it was part of will be discarded altogether.                                                                                                                                                                                                                                                                     |
| `RestreamProposal`     | Requests the application to re-stream a proposal that it has already seen. The application MUST re-publish again all the proposal parts pertaining to that value by sending `NetworkMsg::PublishProposalPart` messages through the `Channels::network` channel.                                                                                                                                                                            |
| `GetHistoryMinHeight`  | Requests the earliest height available in the history maintained by the application. The application MUST respond with its earliest available height.                                                                                                                                                                                                                                                                                      |
| `ReceivedProposalPart` | Notifies the application that consensus has received a proposal part over the network. If this part completes the full proposal, the application MUST respond with the complete proposed value. Otherwise, it MUST respond with `None`.                                                                                                                                                                                                    |                                                                                                                                                                                                                    |
| `GetValidatorSet`      | Requests the validator set for a specific height.                                                                                                                                                                                                                                                                                                                                                                                          |
| `Decided`              | Notifies the application that consensus has decided on a value. This message includes a commit certificate containing the ID of the value that was decided on, the height and round at which it was decided, and the aggregated signatures of the validators that committed to it. In response to this message, the application MAY send a `ConsensusMsg::StartHeight` message back to consensus, instructing it to start the next height. |
| `GetDecidedValue`      | Requests a previously decided value from the application's storage. The application MUST respond with that value if available, or `None` otherwise.                                                                                                                                                                                                                                                                                        |
| `ProcessSyncedValue`   | Notifies the application that a value has been synced from the network. This may happen when the node is catching up with the network. If a value can be decoded from the bytes provided, then the application MUST reply to this message with the decoded value.                                                                                                                                                                          |
| `PeerJoined`  | Notifies the application that a peer has joined our local view of the network. In a gossip network, there is no guarantee that we will ever see all peers, as we are typically only connected to a subset of the network (i.e. in our mesh).                                                                                                                                                                                                                                                                                   |
|`PeerLeft`  | Notifies the application that a peer has left our local view of the network. In a gossip network, there is no guarantee that this means that this peer has left the whole network altogether, just that it is not part of the subset of the network that we are connected to (i.e. our mesh).                                                                                                                                                                                                                                                                                  |

### Application state
The application needs to maintain its internal state so it can react to the messages received from consensus.
Usually, this means implementing mempool, running an RPC server for queries and
submitting transaction or interacting with other parties off-the-network.

This is out of scope for this tutorial, and we make up random values to propose and decide on. Moreover, the application state will be stored in a [redb](https://docs.rs/redb/latest/redb/) database.

## Putting it all together
Now that we have all the context necessary to interact with the Malachite consensus engine, we can start building our
application.

### Create a new Rust project

Let's crate a new empty Rust project with a executable target:

```
$ cargo new --bin tutorial
$ cd tutorial
```

Let's add the dependencies we will eventually need in `Cargo.toml`:

```toml
[package]
name = "tutorial"
version = "1.0.0"
edition = "2021"
publish = false

[dependencies]
# General dependencies
async-trait = "0.1.88"
bytes = { version = "1", default-features = false }
color-eyre = "0.6"
config = { version = "0.14", features = ["toml"], default-features = false }
derive-where = "1.2.7"
eyre = "0.6"
itertools = "0.14"
prost = "0.13"
rand = { version = "0.8.5", features = ["std_rng"] }
redb = "2.4.0"
serde = "1.0"
serde_json = "1.0"
sha3 = "0.10"
thiserror = { version = "2.0", default-features = false }
tokio = "1.44.1"
toml = "0.8.19"
tracing = "0.1.41"

[dependencies.malachitebft-app-channel]
version = "0.0.1"
# This adds the `informalsystems-malachitebft-app-channel` as a dependency, but exposes it
# under `malachitebft_app_channel` instead of its full package name.
git = "https://git@github.com/informalsystems/malachite.git"
package = "informalsystems-malachitebft-app-channel"

[dependencies.malachitebft-proto]
version = "0.0.1"
git = "https://git@github.com/informalsystems/malachite.git"
package = "informalsystems-malachitebft-proto"

[dependencies.malachitebft-test]
version = "0.0.1"
git = "https://git@github.com/informalsystems/malachite.git"
package = "informalsystems-malachitebft-test"

[dependencies.malachitebft-test-cli]
version = "0.0.1"
git = "https://git@github.com/informalsystems/malachite.git"
package = "informalsystems-malachitebft-test-cli"
```

### Application state

Before handling consensus messages, let's start by preparing the application state.

```rust
// src/main.rs

mod state;
mod store;
mod streaming;
```

```rust
// src/state.rs

pub struct State {
    ctx: TestContext,
    signing_provider: Ed25519Provider,
    genesis: Genesis,
    address: Address,
    vote_extensions: HashMap<Height, VoteExtensions<TestContext>>,
    streams_map: PartStreamsMap,
    rng: StdRng,

    pub store: Store,
    pub current_height: Height,
    pub current_round: Round,
    pub current_proposer: Option<Address>,
    pub peers: HashSet<PeerId>,
}
```

We will use two other modules called `store` and `streaming`. The `store` crate provide a database called `Store` which uses [redb](https://docs.rs/redb/latest/redb/) under the hood and offer the following interface:

```rust
    /// Creates a new store/database
    pub fn open(path: impl AsRef<Path>, metrics: DbMetrics) -> Result<Self, StoreError>

    /// Get the minimum height of the decided values in the store
    pub async fn min_decided_value_height(&self) -> Option<Height>

    /// Get the maximum height of the decided values in the store
    pub async fn max_decided_value_height(&self) -> Option<Height>

    /// Get the decided value at the given height
    pub async fn get_decided_value(
        &self,
        height: Height,
    ) -> Result<Option<DecidedValue>, StoreError>

    /// Store a decided value
    pub async fn store_decided_value(
        &self,
        certificate: &CommitCertificate<TestContext>,
        value: Value,
    ) -> Result<(), StoreError>

    /// Store an undecided proposal
    pub async fn store_undecided_proposal(
        &self,
        value: ProposedValue<TestContext>,
    ) -> Result<(), StoreError>

    /// Get the undecided proposal at the given height and round
    pub async fn get_undecided_proposal(
        &self,
        height: Height,
        round: Round,
    ) -> Result<Option<ProposedValue<TestContext>>, StoreError>

    /// Prune the store, removing all decided values below the given height
    pub async fn prune(&self, retain_height: Height) -> Result<Vec<Height>, StoreError>

    /// Remove undecided proposals matching the given value id
    pub async fn remove_undecided_proposals_by_value_id(
        &self,
        value_id: ValueId,
    ) -> Result<(), StoreError>

    /// Get the undecided proposal matching the given value id
    pub async fn get_undecided_proposal_by_value_id(
        &self,
        value_id: ValueId,
    ) -> Result<Option<ProposedValue<TestContext>>, StoreError>
```

Note that the implementation of this store is up to the application developer, who may choose a different underlying database such as RocksDB, LevelDB, or [Fjall](https://github.com/fjall-rs/fjall).

The `streaming` module provides a `PartStreamsMap` data structure. This is used to keep track of the proposal parts that are being streamed over the network. It is also used to re-assemble the full proposal once all parts have been received. It provides the following interface:

```rust
    /// Initialize the data structure
    pub fn new() -> Self

    /// Insert a proposal part into the map, returning the full proposal if all parts have been received
    pub fn insert(
        &mut self,
        peer_id: PeerId,
        msg: StreamMessage<ProposalPart>,
    ) -> Option<ProposalParts>
```

Please refer to [`store`](/code/examples/channel/src/store.rs) and [`streaming`](/code/examples/channel/src/streaming.rs) modules for their full implementation.

Now, we can go through the implementation of the application state. Let's start with helper methods that will be used by the state implementation. Note that the way a proposal is split here is specific to our case (where the value is a natural number and we split it by factoring it into its prime factors). In a real application, the value is likely to be more complex and the splitting logic would be different.

```rust
// src/state.rs

/// Re-assemble a [`ProposedValue`] from its [`ProposalParts`].
///
/// This is done by multiplying all the factors in the parts.
fn assemble_value_from_parts(parts: ProposalParts) -> eyre::Result<ProposedValue<TestContext>> {
    let init = parts.init().ok_or_else(|| eyre!("Missing Init part"))?;

    let value = parts
        .parts
        .iter()
        .filter_map(|part| part.as_data())
        .fold(1, |acc, data| acc * data.factor);

    Ok(ProposedValue {
        height: parts.height,
        round: parts.round,
        valid_round: init.pol_round,
        proposer: parts.proposer,
        value: Value::new(value),
        validity: Validity::Valid,
    })
}

/// Decodes a Value from its byte representation using ProtobufCodec
pub fn decode_value(bytes: Bytes) -> Value {
    ProtobufCodec.decode(bytes).unwrap()
}

/// Returns the list of prime factors of the given value
///
/// In a real application, this would typically split transactions
/// into chunks ino order to reduce bandwidth requirements due
/// to duplication of gossip messages.
fn factor_value(value: Value) -> Vec<u64> {
    let mut factors = Vec::new();
    let mut n = value.value;

    let mut i = 2;
    while i * i <= n {
        if n % i == 0 {
            factors.push(i);
            n /= i;
        } else {
            i += 1;
        }
    }

    if n > 1 {
        factors.push(n);
    }

    factors
}

```

Then, let's introduce _getter_ methods that are quite self-explanatory:

```rust
impl State {
    // ...

    // Returns the earliest height available in the state
    pub async fn get_earliest_height(&self) -> Height {
        self.store
            .min_decided_value_height()
            .await
            .unwrap_or_default()
    }

    // Retrieves a decided block at the given height
    pub async fn get_decided_value(&self, height: Height) -> Option<DecidedValue> {
        self.store.get_decided_value(height).await.ok().flatten()
    }

    // Retrieves a previously built proposal value for the given height
    pub async fn get_previously_built_value(
        &self,
        height: Height,
        round: Round,
    ) -> eyre::Result<Option<LocallyProposedValue<TestContext>>> {
        let Some(proposal) = self.store.get_undecided_proposal(height, round).await? else {
            return Ok(None);
        };

        Ok(Some(LocallyProposedValue::new(
            proposal.height,
            proposal.round,
            proposal.value,
        )))
    }

    // Returns the set of validators.
    pub fn get_validator_set(&self) -> &ValidatorSet {
        &self.genesis.validator_set
    }

    // ...
}
```

Now, let's see how value proposition works in the application state. It exposes the function `propose_value` which creates a new proposal value for the given height and round. In our example, the proposed value is a randomly generated integer.

```rust
impl State {
    // ...

    /// Creates a new proposal value for the given height
    async fn create_proposal(
        &mut self,
        height: Height,
        round: Round,
    ) -> eyre::Result<ProposedValue<TestContext>> {
        assert_eq!(height, self.current_height);
        assert_eq!(round, self.current_round);

        // We create a new value.
        let value = self.make_value(height, round);

        // Simulate some processing time
        sleep(Duration::from_millis(500)).await;

        let proposal = ProposedValue {
            height,
            round,
            valid_round: Round::Nil,
            proposer: self.address, // We are the proposer
            value,
            validity: Validity::Valid, // Our proposals are de facto valid
        };

        // Insert the new proposal into the undecided proposals.
        self.store
            .store_undecided_proposal(proposal.clone())
            .await?;

        Ok(proposal)
    }

    /// Make up a new value to propose
    /// A real application would have a more complex logic here,
    /// typically reaping transactions from a mempool and executing them against its state,
    /// before computing the merkle root of the new app state.
    fn make_value(&mut self, height: Height, _round: Round) -> Value {
        let value = self.rng.gen_range(100..=100000);

        let extensions = self
            .vote_extensions
            .remove(&height)
            .unwrap_or_default()
            .extensions
            .into_iter()
            .map(|(_, e)| e.message)
            .fold(BytesMut::new(), |mut acc, e| {
                acc.extend_from_slice(&e);
                acc
            })
            .freeze();

        Value { value, extensions }
    }

    /// Creates a new proposal value for the given height
    /// Returns either a previously built proposal or creates a new one
    pub async fn propose_value(
        &mut self,
        height: Height,
        round: Round,
    ) -> eyre::Result<LocallyProposedValue<TestContext>> {
        assert_eq!(height, self.current_height);
        assert_eq!(round, self.current_round);

        // Check if we have already built a proposal for this height and round
        if let Some(proposal) = self.get_previously_built_value(height, round).await? {
            return Ok(proposal);
        }

        let proposal = self.create_proposal(height, round).await?;

        Ok(LocallyProposedValue::new(
            proposal.height,
            proposal.round,
            proposal.value,
        ))
    }

    // ...
}
```

Then, we need to stream this proposal. For that, the proposal need to be split into proposal parts. This is the role of the `stream_proposal` method. It leverages the `stream_id` and `value_to_parts` methods to create the final stream message. Note that the last part of the proposal contains the signature of the hash of the proposal parts. Moreover, each part has an associated sequence number; this number is used by the `PartStreamsMap` data structure to re-assemble the full proposal in the correct order.

```rust
impl State {
    // ...

    /// Returns the stream id for the current height and round
    fn stream_id(&self) -> StreamId {
        let mut bytes = Vec::with_capacity(size_of::<u64>() + size_of::<u32>());
        bytes.extend_from_slice(&self.current_height.as_u64().to_be_bytes());
        bytes.extend_from_slice(&self.current_round.as_u32().unwrap().to_be_bytes());
        StreamId::new(bytes.into())
    }

    /// Converts a locally proposed value into a list of proposal parts
    fn value_to_parts(
        &self,
        value: LocallyProposedValue<TestContext>,
        pol_round: Round,
    ) -> Vec<ProposalPart> {
        let mut hasher = sha3::Keccak256::new();
        let mut parts = Vec::new();

        // Init
        // Include metadata about the proposal
        {
            parts.push(ProposalPart::Init(ProposalInit::new(
                value.height,
                value.round,
                pol_round,
                self.address,
            )));

            hasher.update(value.height.as_u64().to_be_bytes().as_slice());
            hasher.update(value.round.as_i64().to_be_bytes().as_slice());
        }

        // Data
        // Include each prime factor of the value as a separate proposal part
        {
            for factor in factor_value(value.value) {
                parts.push(ProposalPart::Data(ProposalData::new(factor)));

                hasher.update(factor.to_be_bytes().as_slice());
            }
        }

        // Fin
        // Sign the hash of the proposal parts
        {
            let hash = hasher.finalize().to_vec();
            let signature = self.signing_provider.sign(&hash);
            parts.push(ProposalPart::Fin(ProposalFin::new(signature)));
        }

        parts
    }

    /// Creates a stream of messages containing a proposal parts.
    /// Updates internal sequence number and current proposal.
    pub fn stream_proposal(
        &mut self,
        value: LocallyProposedValue<TestContext>,
        pol_round: Round,
    ) -> impl Iterator<Item = StreamMessage<ProposalPart>> {
        let parts = self.value_to_parts(value, pol_round);
        let stream_id = self.stream_id();

        let mut msgs = Vec::with_capacity(parts.len() + 1);
        let mut sequence = 0;

        for part in parts {
            let msg = StreamMessage::new(stream_id.clone(), sequence, StreamContent::Data(part));
            sequence += 1;
            msgs.push(msg);
        }

        msgs.push(StreamMessage::new(stream_id, sequence, StreamContent::Fin));

        msgs.into_iter()
    }

    // ...
}
```

Then, when receiving the proposal part, the function `received_proposal_part` inserts it into the `PartStreamsMap` and tries to assemble the full proposal. If the proposal is indeed full, we check if it is outdated and then verify its signature. If the signature is valid, we store the proposal in the undecided proposals and return it. Otherwise, we log an error and return `None`.

```rust
/// Represents errors that can occur during the verification of a proposal's signature.
#[derive(Debug)]
enum SignatureVerificationError {
    /// Indicates that the `Init` part of the proposal is unexpectedly missing.
    MissingInitPart,

    /// Indicates that the `Fin` part of the proposal is unexpectedly missing.
    MissingFinPart,

    /// Indicates that the proposer was not found in the validator set.
    ProposerNotFound,

    /// Indicates that the signature in the `Fin` part is invalid.
    InvalidSignature,
}

impl State {
    // ...

    /// Verifies the signature of the proposal.
    /// Returns `Ok(())` if the signature is valid, or an appropriate `SignatureVerificationError`.
    fn verify_proposal_signature(
        &self,
        parts: &ProposalParts,
    ) -> Result<(), SignatureVerificationError> {
        let mut hasher = sha3::Keccak256::new();

        let init = parts
            .init()
            .ok_or(SignatureVerificationError::MissingInitPart)?;

        let fin = parts
            .fin()
            .ok_or(SignatureVerificationError::MissingFinPart)?;

        let hash = {
            hasher.update(init.height.as_u64().to_be_bytes());
            hasher.update(init.round.as_i64().to_be_bytes());

            // The correctness of the hash computation relies on the parts being ordered by sequence
            // number, which is guaranteed by the `PartStreamsMap`.
            for part in parts.parts.iter().filter_map(|part| part.as_data()) {
                hasher.update(part.factor.to_be_bytes());
            }

            hasher.finalize()
        };

        // Retrieve the the proposer
        let proposer = self
            .get_validator_set()
            .get_by_address(&parts.proposer)
            .ok_or(SignatureVerificationError::ProposerNotFound)?;

        // Verify the signature
        if !self
            .signing_provider
            .verify(&hash, &fin.signature, &proposer.public_key)
        {
            return Err(SignatureVerificationError::InvalidSignature);
        }

        Ok(())
    }

    /// Processes and adds a new proposal to the state if it's valid
    /// Returns Some(ProposedValue) if the proposal was accepted, None otherwise
    pub async fn received_proposal_part(
        &mut self,
        from: PeerId,
        part: StreamMessage<ProposalPart>,
    ) -> eyre::Result<Option<ProposedValue<TestContext>>> {
        let sequence = part.sequence;

        // Check if we have a full proposal
        let Some(parts) = self.streams_map.insert(from, part) else {
            return Ok(None);
        };

        // Check if the proposal is outdated
        if parts.height < self.current_height {
            debug!(
                height = %self.current_height,
                round = %self.current_round,
                part.height = %parts.height,
                part.round = %parts.round,
                part.sequence = %sequence,
                "Received outdated proposal part, ignoring"
            );

            return Ok(None);
        }

        // Verify the proposal signature
        match self.verify_proposal_signature(&parts) {
            Ok(()) => {
                // Signature verified successfully, continue processing
            }
            Err(SignatureVerificationError::MissingInitPart) => {
                return Err(eyre!(
                    "Expected to have full proposal but `Init` proposal part is missing for proposer: {}",
                    parts.proposer
                ));
            }
            Err(SignatureVerificationError::MissingFinPart) => {
                return Err(eyre!(
                    "Expected to have full proposal but `Fin` proposal part is missing for proposer: {}",
                    parts.proposer
                ));
            }
            Err(SignatureVerificationError::ProposerNotFound) => {
                error!(proposer = %parts.proposer, "Proposer not found in validator set");
                return Ok(None);
            }
            Err(SignatureVerificationError::InvalidSignature) => {
                error!(proposer = %parts.proposer, "Invalid signature in Fin part");
                return Ok(None);
            }
        }

        // Re-assemble the proposal from its parts
        let value = assemble_value_from_parts(parts)?;

        info!(
            "Storing undecided proposal {} {}",
            value.height, value.round
        );

        self.store.store_undecided_proposal(value.clone()).await?;

        Ok(Some(value))
    }

    // ...
}
```

Finally, the `commit` methods commits a value with the given certificate and moves to the next height by doing a bit of cleanup.

```rust
/// Number of historical values to keep in the store
const HISTORY_LENGTH: u64 = 100;

impl State {
    // ...

    /// Commits a value with the given certificate, updating internal state
    /// and moving to the next height
    pub async fn commit(
        &mut self,
        certificate: CommitCertificate<TestContext>,
        extensions: VoteExtensions<TestContext>,
    ) -> eyre::Result<()> {
        let (height, round, value_id) =
            (certificate.height, certificate.round, certificate.value_id);

        // Store extensions for use at next height if we are the proposer
        self.vote_extensions.insert(height.increment(), extensions);

        // Get the first proposal with the given value id. There may be multiple identical ones
        // if peers have restreamed at different rounds.
        let Ok(Some(proposal)) = self
            .store
            .get_undecided_proposal_by_value_id(value_id)
            .await
        else {
            return Err(eyre!(
                "Trying to commit a value with value id {value_id} at height {height} and round {round} for which there is no proposal"
            ));
        };

        self.store
            .store_decided_value(&certificate, proposal.value)
            .await?;

        // Remove all proposals with the given value id.
        self.store
            .remove_undecided_proposals_by_value_id(value_id)
            .await?;

        // Prune the store, keep the last HISTORY_LENGTH values
        let retain_height = Height::new(height.as_u64().saturating_sub(HISTORY_LENGTH));
        self.store.prune(retain_height).await?;

        // Move to next height
        self.current_height = self.current_height.increment();
        self.current_round = Round::new(0);

        Ok(())
    }

    // ...
}
```

Finally, let's define the `State` constructor:

```rust
// Make up a seed for the rng based on our address in
// order for each node to likely propose different values at
// each round.
fn seed_from_address(address: &Address) -> u64 {
    address.into_inner().chunks(8).fold(0u64, |acc, chunk| {
        let term = chunk.iter().fold(0u64, |acc, &x| {
            acc.wrapping_shl(8).wrapping_add(u64::from(x))
        });
        acc.wrapping_add(term)
    })
}

impl State {
    // ...

    /// Creates a new State instance with the given validator address and starting height
    pub fn new(
        ctx: TestContext,
        signing_provider: Ed25519Provider,
        genesis: Genesis,
        address: Address,
        height: Height,
        store: Store,
    ) -> Self {
        Self {
            ctx,
            signing_provider,
            genesis,
            current_height: height,
            current_round: Round::new(0),
            current_proposer: None,
            address,
            store,
            vote_extensions: HashMap::new(),
            streams_map: PartStreamsMap::new(),
            rng: StdRng::seed_from_u64(seed_from_address(&address)),
            peers: HashSet::new(),
        }
    }

    // ...
}
```

### The consensus dialog

As seen above, messages sent by the engine have a `reply` field that the application
can use to respond to the message. Since the flow of messages might not be particularly
explicit from the code, here is a diagram showing the flow of messages and the replies expected by the engine,
in the case where we are the proposer and when are simply a validator.

```mermaid
sequenceDiagram

   alt Startup
   Consensus->>Application: ConsensusReady
   activate Application
   note right of Application: Find start height
   Application-->>Consensus: StartHeight
   deactivate Application
   end

   alt Generic updates
   Consensus->>Application: StartedRound
   note right of Application: Update internal state
   else
   Consensus->>Application: GetHistoryMinHeight
   activate Application
   note right of Application: Find earliest height stored
   Application->>Consensus: Height
   deactivate Application
   else
   Consensus->>Application: GetValidatorSet
   activate Application
   note right of Application: Gather validator set
   Application->>Consensus: ValidatorSet
   deactivate Application
   else
   Consensus->>Application: GetDecidedValue
   activate Application
   note right of Application: Find decided value
   Application->>Consensus: DecidedValue
   deactivate Application
   end

   alt Proposer
   Consensus->>Application: GetValue
   activate Application
   note right of Application: Send previously compiled value or create new one
   Application->>Consensus: LocallyProposedValue
   deactivate Application
   activate Application
   Application-->>Network: PublishProposalPart
   deactivate Application
   note right of Application: Publish new value to other nodes on network
   end

   alt Validator
   Consensus->>Application: ReceivedProposalPart
   activate Application
   note right of Application: Try to compile proposal from parts
   Application->>Consensus: ProposedValue
   deactivate Application
   else
   Consensus->>Application: Decided
   activate Application
   note right of Application: Store certificate in state<br>Start next height
   Application->>Consensus: StartHeight
   deactivate Application
   else
   Consensus->>Application: ProcessSyncedValue
   activate Application
   note right of Application: Decode received value
   Application->>Consensus: ProposedValue
   deactivate Application
   end
```

### Handle application messages

Now that we have the application state, we can start handling messages from the consensus (referred to as application messages), and act on those accordingly.

Let's define a `run` function in a new `app` module in `src/app.rs`, which will wait for messages from consensus
and handle those by updating its state and sending back the appropriate responses.

```rust
// src/main.rs

mod app;
```

```rust
// src/app.rs

pub async fn run(state: &mut State, channels: &mut Channels<TestContext>) -> eyre::Result<()> {
    while let Some(msg) = channels.consensus.recv().await {
        match msg {
            // Handle application messages
        }
    }
}
```

The first message to handle is the `ConsensusReady` message, signaling to the app
that Malachite is ready to start consensus.

We can simply respond by telling the engine to start consensus at the current height,
which is either 1 or the next height after the last decided value in the store (typically when recovering from a crash).

```rust
            AppMsg::ConsensusReady { reply, .. } => {
                let start_height = state
                    .store
                    .max_decided_value_height()
                    .await
                    .map(|height| height.increment())
                    .unwrap_or_else(|| Height::INITIAL);

                info!(%start_height, "Consensus is ready");

                sleep(Duration::from_millis(200)).await;

                if reply
                    .send((start_height, state.get_validator_set().clone()))
                    .is_err()
                {
                    error!("Failed to send ConsensusReady reply");
                }
            }
```

The next message to handle is the `StartRound` message, signaling to the app
that consensus has entered a new round (including the initial round 0).

We can use that opportunity to update our internal state. Moreover, if we have already built or seen a value for this height and round, we can send it back to consensus. This may happen when we are restarting after a crash.

```rust
            AppMsg::StartedRound {
                height,
                round,
                proposer,
                reply_value,
            } => {
                info!(%height, %round, %proposer, "Started round");

                reload_log_level(height, round);

                state.current_height = height;
                state.current_round = round;
                state.current_proposer = Some(proposer);

                if let Some(proposal) = state.store.get_undecided_proposal(height, round).await? {
                    info!(%height, %round, "Replaying already known proposed value: {}", proposal.value.id());

                    if reply_value.send(Some(proposal)).is_err() {
                        error!("Failed to send undecided proposal");
                    }
                } else {
                    let _ = reply_value.send(None);
                }
            }

// ...

/// Reload the tracing subscriber log level based on the current height and round.
/// This is useful to increase the log level when debugging a specific height and round.
fn reload_log_level(_height: Height, round: Round) {
    use malachitebft_test_cli::logging;

    if round.as_i64() > 0 {
        logging::reload(logging::LogLevel::Debug);
    } else {
        logging::reset();
    }
}
```

At some point, we may end up being the proposer for that round, and the engine
will then ask us for a value to propose to the other validators.

Here, it is important that, if we have previously built a value for this height and round, we send back the very same value. Otherwise, we need to create a new value to propose and send it back to consensus.

> [!NOTE]
> We can ignore the timeout as we are building the value right away.
> If we were let's say reaping as many txes from a mempool and executing them,
> then we would need to respect the timeout and stop at a certain point.

After proposing the value, we need to break it down into parts and stream them over the network to our peers.

> [!NOTE]
In this tutorial, the value is simply an integer and therefore results in a very small
message to gossip over the network, but if we were building a real application,
say building blocks containing thousands of transactions, the proposal would typically only
carry the block hash and the full block itself would be split into parts in order to
avoid blowing up the bandwidth requirements by gossiping a single huge message.

```rust
            AppMsg::GetValue {
                height,
                round,
                timeout: _,
                reply,
            } => {
                info!(%height, %round, "Consensus is requesting a value to propose");

                let proposal = match state.get_previously_built_value(height, round).await? {
                    Some(proposal) => {
                        info!(value = %proposal.value.id(), "Re-using previously built value");
                        proposal
                    }
                    None => {
                        info!("Building a new value to propose");
                        state.propose_value(height, round).await?
                    }
                };

                if reply.send(proposal.clone()).is_err() {
                    error!("Failed to send GetValue reply");
                }

                // The POL round is always nil when we propose a newly built value.
                // See L15/L18 of the Tendermint algorithm.
                let pol_round = Round::Nil;

                for stream_message in state.stream_proposal(proposal, pol_round) {
                    info!(%height, %round, "Streaming proposal part: {stream_message:?}");

                    channels
                        .network
                        .send(NetworkMsg::PublishProposalPart(stream_message))
                        .await?;
                }
            }
```

On the receiving end of these proposal parts (ie. when we are not the proposer),
we need to process these parts and re-assemble the full value.
To this end, we store each part that we receive and assemble the full value once we
have all its constituent parts. Then we send that value back to consensus for it to
consider and vote for or against it (ie. vote `nil`), depending on its validity.

```rust
            AppMsg::ReceivedProposalPart { from, part, reply } => {
                let part_type = match &part.content {
                    StreamContent::Data(part) => part.get_type(),
                    StreamContent::Fin => "end of stream",
                };

                info!(%from, %part.sequence, part.type = %part_type, "Received proposal part");

                let proposed_value = state.received_proposal_part(from, part).await?;

                if reply.send(proposed_value).is_err() {
                    error!("Failed to send ReceivedProposalPart reply");
                }
            }
```

It is also possible that the application is requested to restream a proposal it has already seen.

```rust
            AppMsg::RestreamProposal {
                height,
                round,
                valid_round,
                address: _,
                value_id: _,
            } => {
                info!(%height, %valid_round, "Restreaming existing propos*al...");

                let proposal = state
                    .store
                    .get_undecided_proposal(height, valid_round)
                    .await?;

                if let Some(proposal) = proposal {
                    let locally_proposed_value = LocallyProposedValue {
                        height,
                        round,
                        value: proposal.value,
                    };

                    for stream_message in state.stream_proposal(locally_proposed_value, valid_round)
                    {
                        info!(%height, %valid_round, "Publishing proposal part: {stream_message:?}");

                        channels
                            .network
                            .send(NetworkMsg::PublishProposalPart(stream_message))
                            .await?;
                    }
                }
            }
```

When consensus is preparing to send a pre-commit vote, it first calls `ExtendVote`, asking the application to returns a blob of data called a vote extension. This data is opaque to the consensus algorithm but can contain application-specific information. The proposer of the next block will receive all vote extensions along with the commit certificate.

In our case, the vote extension is empty.

```rust
            AppMsg::ExtendVote {
                height: _,
                round: _,
                value_id: _,
                reply,
            } => {
                if reply.send(None).is_err() {
                    error!("Failed to send ExtendVote reply");
                }
            }
```

The application is also responsible to verify a given vote extension. In our case, we simply return `OK(())`.

```rust
            AppMsg::VerifyVoteExtension {
                height: _,
                round: _,
                value_id: _,
                extension: _,
                reply,
            } => {
                if reply.send(Ok(())).is_err() {
                    error!("Failed to send VerifyVoteExtension reply");
                }
            }
```

In some cases, e.g. to verify the signature of a vote received at a higher height
than the one we are at (e.g. because we are lagging behind a little bit),
the engine may ask us for the validator set at that height.

In our case, our validator set stays constant between heights so we can
send back the validator set found in our genesis state.

```rust
            AppMsg::GetValidatorSet { height: _, reply } => {
                if reply.send(state.get_validator_set().clone()).is_err() {
                    error!("Failed to send GetValidatorSet reply");
                }
            }
```

As just mentioned, it may happen that our node is lagging behind its peers. In that case,
a synchronization mechanism will automatically kick to try and catch up to
our peers. When that happens, some of these peers will send us decided values
for the heights in between the one we are currently at (included) and the one
that they are at. When the engine receives such a value, it will forward to the application
to decode it from its wire format and send back the decoded value to consensus.

```rust
            AppMsg::ProcessSyncedValue {
                height,
                round,
                proposer,
                value_bytes,
                reply,
            } => {
                info!(%height, %round, "Processing synced value");

                let value = decode_value(value_bytes);
                let proposed_value = ProposedValue {
                    height,
                    round,
                    valid_round: Round::Nil,
                    proposer,
                    value,
                    validity: Validity::Valid,
                };

                state
                    .store
                    .store_undecided_proposal(proposed_value.clone())
                    .await?;

                if reply.send(proposed_value).is_err() {
                    error!("Failed to send ProcessSyncedValue reply");
                }
            }
```

After some time, consensus will finally reach a decision on the value
to commit for the current height, and will notify the application,
providing it with a commit certificate which contains the ID of the value
that was decided on as well as the set of commits for that value,
ie. the precommits together with their (aggregated) signatures.

When that happens, we store the decided value in our store,
and instruct consensus to start the next height.

```rust
            AppMsg::Decided {
                certificate,
                extensions,
                reply,
            } => {
                info!(
                    height = %certificate.height,
                    round = %certificate.round,
                    value = %certificate.value_id,
                    "Consensus has decided on value"
                );

                state.commit(certificate, extensions).await?;

                if reply
                    .send(ConsensusMsg::StartHeight(
                        state.current_height,
                        state.get_validator_set().clone(),
                    ))
                    .is_err()
                {
                    error!("Failed to send Decided reply");
                }
            }
```

If, on the other hand, we are not lagging behind but are instead asked by one of
our peer to help them catch up because they are the one lagging behind,
then the engine might ask the application to provide with the value
that was decided at some lower height. In that case, we fetch it from our store
and send it to consensus.

```rust
            AppMsg::GetDecidedValue { height, reply } => {
                info!(%height, "Received sync request for decided value");

                let decided_value = state.get_decided_value(height).await;
                info!(%height, "Found decided value: {decided_value:?}");

                let raw_decided_value = decided_value.map(|decided_value| RawDecidedValue {
                    certificate: decided_value.certificate,
                    value_bytes: ProtobufCodec.encode(&decided_value.value).unwrap(),
                });

                if reply.send(raw_decided_value).is_err() {
                    error!("Failed to send GetDecidedValue reply");
                }
            }
```

In order to figure out if we can help a peer that is lagging behind,
the engine may ask us for the height of the earliest available value in our store.

```rust
            AppMsg::GetHistoryMinHeight { reply } => {
                let min_height = state.get_earliest_height().await;

                if reply.send(min_height).is_err() {
                    error!("Failed to send GetHistoryMinHeight reply");
                }
            }
```

Finally, the application is informed about other peers joining or leaving the local view of the network.

```rust
            AppMsg::PeerJoined { peer_id } => {
                info!(%peer_id, "Peer joined our local view of network");

                state.peers.insert(peer_id);
            }

            AppMsg::PeerLeft { peer_id } => {
                info!(%peer_id, "Peer left our local view of network");

                state.peers.remove(&peer_id);
            }
```

Ideally, the consensus actor should never die, but if it does, we can only return an error.

```rust
    // End of while loop
    }

    Err(eyre!("Consensus channel closed unexpectedly"))
}
```

### Node

Now, it is time to define the main application struct as well as the handle implementing the `NodeHandle` trait.

```rust
// src/main.rs

mod app;

// ...
```

```rust
// src/node.rs

#[derive(Clone)]
pub struct App {
    pub home_dir: PathBuf,
    pub config_file: PathBuf,
    pub genesis_file: PathBuf,
    pub private_key_file: PathBuf,
    pub start_height: Option<Height>,
}

pub struct Handle {
    pub app: JoinHandle<()>,
    pub engine: EngineHandle,
    pub tx_event: TxEvent<TestContext>,
}
```

The handle is quite self-explanatory:

```rust
#[async_trait]
impl NodeHandle<TestContext> for Handle {
    fn subscribe(&self) -> RxEvent<TestContext> {
        self.tx_event.subscribe()
    }

    async fn kill(&self, _reason: Option<String>) -> eyre::Result<()> {
        self.engine.actor.kill_and_wait(None).await?;
        self.app.abort();
        self.engine.handle.abort();
        Ok(())
    }
}
```

The `App` struct is a bit more complex. It implements the following traits:

- `Node` is the main trait defining, especially, the methods `start` and `run` that are used to start the node. The implementation makes use of two other crates: `config` and `metrics`. These are not the most important components of this tutorial, please refer to their respective documentation for more information: [config](/code/examples/channel/src/config.rs) and [metrics](/code/examples/channel/src/metrics.rs). The `start` method loads the configuration, the private key, the genesis, and starts the engine. It also initializes the store and the state. The `run` method is a simple wrapper around `start` that lets the node run until it is stopped (ideally never). The other methods are quite straightforward.
- `CanMakeGenesis` is used to create genesis information.
- `CanGeneratePrivateKey` is used to generate a private key.
- `CanMakePrivateKeyFile` is used to create a private key file.
- `CanMakeConfig` is used to create a configuration.

```rust
#[async_trait]
impl Node for App {
    type Context = TestContext;
    type Config = Config;
    type Genesis = Genesis;
    type PrivateKeyFile = PrivateKey;
    type SigningProvider = Ed25519Provider;
    type NodeHandle = Handle;

    fn get_home_dir(&self) -> PathBuf {
        self.home_dir.to_owned()
    }

    fn load_config(&self) -> eyre::Result<Self::Config> {
        load_config(&self.config_file, Some("MALACHITE"))
    }

    fn get_address(&self, pk: &PublicKey) -> Address {
        Address::from_public_key(pk)
    }

    fn get_public_key(&self, pk: &PrivateKey) -> PublicKey {
        pk.public_key()
    }

    fn get_keypair(&self, pk: PrivateKey) -> Keypair {
        Keypair::ed25519_from_bytes(pk.inner().to_bytes()).unwrap()
    }

    fn load_private_key(&self, file: Self::PrivateKeyFile) -> PrivateKey {
        file
    }

    fn load_private_key_file(&self) -> eyre::Result<Self::PrivateKeyFile> {
        let private_key = std::fs::read_to_string(&self.private_key_file)?;
        serde_json::from_str(&private_key).map_err(Into::into)
    }

    fn get_signing_provider(&self, private_key: PrivateKey) -> Self::SigningProvider {
        Ed25519Provider::new(private_key)
    }

    fn load_genesis(&self) -> eyre::Result<Self::Genesis> {
        let genesis = std::fs::read_to_string(&self.genesis_file)?;
        serde_json::from_str(&genesis).map_err(Into::into)
    }

    async fn start(&self) -> eyre::Result<Handle> {
        let config = self.load_config()?;

        let span = tracing::error_span!("node", moniker = %config.moniker);
        let _enter = span.enter();

        let private_key_file = self.load_private_key_file()?;
        let private_key = self.load_private_key(private_key_file);
        let public_key = self.get_public_key(&private_key);
        let address = self.get_address(&public_key);
        let signing_provider = self.get_signing_provider(private_key);
        let ctx = TestContext::new();

        let genesis = self.load_genesis()?;
        let initial_validator_set = genesis.validator_set.clone();

        let codec = ProtobufCodec;

        let (mut channels, engine_handle) = malachitebft_app_channel::start_engine(
            ctx,
            codec,
            self.clone(),
            config.clone(),
            self.start_height,
            initial_validator_set,
        )
        .await?;

        let tx_event = channels.events.clone();

        let registry = SharedRegistry::global().with_moniker(&config.moniker);
        let metrics = DbMetrics::register(&registry);

        if config.metrics.enabled {
            tokio::spawn(metrics::serve(config.metrics.listen_addr));
        }

        let db_dir = self.get_home_dir().join("db");
        std::fs::create_dir_all(&db_dir)?;

        let store = Store::open(db_dir.join("store.db"), metrics)?;
        let start_height = self.start_height.unwrap_or(Height::INITIAL);
        let mut state = State::new(ctx, signing_provider, genesis, address, start_height, store);

        let span = tracing::error_span!("node", moniker = %config.moniker);
        let app_handle = tokio::spawn(
            async move {
                if let Err(e) = crate::app::run(&mut state, &mut channels).await {
                    tracing::error!(%e, "Application error");
                }
            }
            .instrument(span),
        );

        Ok(Handle {
            app: app_handle,
            engine: engine_handle,
            tx_event,
        })
    }

    async fn run(self) -> eyre::Result<()> {
        let handles = self.start().await?;
        handles.app.await.map_err(Into::into)
    }
}

impl CanMakeGenesis for App {
    fn make_genesis(&self, validators: Vec<(PublicKey, VotingPower)>) -> Self::Genesis {
        let validators = validators
            .into_iter()
            .map(|(pk, vp)| Validator::new(pk, vp));

        let validator_set = ValidatorSet::new(validators);

        Genesis { validator_set }
    }
}

impl CanGeneratePrivateKey for App {
    fn generate_private_key<R>(&self, rng: R) -> PrivateKey
    where
        R: RngCore + CryptoRng,
    {
        PrivateKey::generate(rng)
    }
}

impl CanMakePrivateKeyFile for App {
    fn make_private_key_file(&self, private_key: PrivateKey) -> Self::PrivateKeyFile {
        private_key
    }
}

impl CanMakeConfig for App {
    fn make_config(index: usize, total: usize, settings: MakeConfigSettings) -> Self::Config {
        make_config(index, total, settings)
    }
}

/// Generate configuration for node "index" out of "total" number of nodes.
fn make_config(index: usize, total: usize, settings: MakeConfigSettings) -> Config {
    use itertools::Itertools;
    use rand::seq::IteratorRandom;
    use rand::Rng;

    use malachitebft_app_channel::app::config::*;

    const CONSENSUS_BASE_PORT: usize = 27000;
    const METRICS_BASE_PORT: usize = 29000;

    let consensus_port = CONSENSUS_BASE_PORT + index;
    let metrics_port = METRICS_BASE_PORT + index;

    Config {
        moniker: format!("app-{}", index),
        consensus: ConsensusConfig {
            // Current channel app does not support proposal-only value payload properly as Init does not include valid_round
            value_payload: ValuePayload::ProposalAndParts,
            vote_sync: VoteSyncConfig {
                mode: VoteSyncMode::RequestResponse,
            },
            timeouts: TimeoutConfig::default(),
            p2p: P2pConfig {
                protocol: PubSubProtocol::default(),
                listen_addr: settings.transport.multiaddr("127.0.0.1", consensus_port),
                persistent_peers: if settings.discovery.enabled {
                    let mut rng = rand::thread_rng();
                    let count = if total > 1 {
                        rng.gen_range(1..=(total / 2))
                    } else {
                        0
                    };
                    let peers = (0..total)
                        .filter(|j| *j != index)
                        .choose_multiple(&mut rng, count);

                    peers
                        .iter()
                        .unique()
                        .map(|index| {
                            settings
                                .transport
                                .multiaddr("127.0.0.1", CONSENSUS_BASE_PORT + index)
                        })
                        .collect()
                } else {
                    (0..total)
                        .filter(|j| *j != index)
                        .map(|j| {
                            settings
                                .transport
                                .multiaddr("127.0.0.1", CONSENSUS_BASE_PORT + j)
                        })
                        .collect()
                },
                discovery: settings.discovery,
                transport: settings.transport,
                ..Default::default()
            },
        },
        metrics: MetricsConfig {
            enabled: true,
            listen_addr: format!("127.0.0.1:{metrics_port}").parse().unwrap(),
        },
        runtime: settings.runtime,
        logging: LoggingConfig::default(),
        value_sync: ValueSyncConfig::default(),
    }
}
```

### Logging

It is up to the application integrator to implement logging. However, given that Malachite uses the [`tracing`](https://crates.io/crates/tracing) library for logging internally, it is natural to use it as well for the application, so we will just do that by using the `logging` module form the `malachitebft-test-cli` crate.

The initialization of the logger is shown in the next section with the main function.

### Command-line interface
Most applications will expect to receive arguments over the command-line, eg. to point it at a configuration file.
This is outside the scope of Malachite, but for the purpose of this tutorial we can use Malachite's test CLI instead of creating our own.

First, let's define the main function of the program. We first parse the command-line arguments and then execute the appropriate command.

```rust
use malachitebft_test_cli::config::{LogFormat, LogLevel};
use malachitebft_test_cli::args::{Args, Commands};
use malachitebft_test_cli::cmd::init::InitCmd;
use malachitebft_test_cli::cmd::start::StartCmd;
use malachitebft_test_cli::cmd::testnet::TestnetCmd;
use malachitebft_test_cli::cmd::dump_wal::DumpWalCmd;
use malachitebft_test_cli::{logging, runtime};

fn main() -> Result<()> {
    color_eyre::install()?;

    // Load command-line arguments and possible configuration file.
    let args = Args::new();

    // Parse the input command.
    match &args.command {
        Commands::Start(cmd) => start(&args, cmd),
        Commands::Init(cmd) => init(&args, cmd),
        Commands::Testnet(cmd) => testnet(&args, cmd),
        Commands::DumpWal(cmd) => dump_wal(&args, cmd),
        Commands::DistributedTestnet(_) => unimplemented!(),
    }
}
```

The first command is the `Start` command. It is used to start the application.

```rust
fn start(args: &Args, cmd: &StartCmd) -> Result<()> {
    // Setup the application
    let app = App {
        home_dir: args.get_home_dir()?,
        config_file: args.get_config_file_path()?,
        genesis_file: args.get_genesis_file_path()?,
        private_key_file: args.get_priv_validator_key_file_path()?,
        start_height: cmd.start_height.map(Height::new),
    };

    let config: Config = app.load_config()?;

    // This is a drop guard responsible for flushing any remaining logs when the program terminates.
    // It must be assigned to a binding that is not _, as _ will result in the guard being dropped immediately.
    let _guard = logging::init(config.logging.log_level, config.logging.log_format);

    let rt = runtime::build_runtime(config.runtime)?;

    info!(moniker = %config.moniker, "Starting Malachite");

    // Start the node
    rt.block_on(app.run())
        .map_err(|error| eyre!("Failed to run the application node: {error}"))
}
```

The `Init` command is used to create a new configuration file.

```rust
fn init(args: &Args, cmd: &InitCmd) -> Result<()> {
    // This is a drop guard responsible for flushing any remaining logs when the program terminates.
    // It must be assigned to a binding that is not _, as _ will result in the guard being dropped immediately.
    let _guard = logging::init(LogLevel::Info, LogFormat::Plaintext);

    // Setup the application
    let app = App {
        home_dir: args.get_home_dir()?,
        config_file: args.get_config_file_path()?,
        genesis_file: args.get_genesis_file_path()?,
        private_key_file: args.get_priv_validator_key_file_path()?,
        start_height: None,
    };

    cmd.run(
        &app,
        &args.get_config_file_path()?,
        &args.get_genesis_file_path()?,
        &args.get_priv_validator_key_file_path()?,
    )
    .map_err(|error| eyre!("Failed to run init command {error:?}"))
}
```

The `Testnet` command is used to create a testnet configuration.

```rust
fn testnet(args: &Args, cmd: &TestnetCmd) -> Result<()> {
    // This is a drop guard responsible for flushing any remaining logs when the program terminates.
    // It must be assigned to a binding that is not _, as _ will result in the guard being dropped immediately.
    let _guard = logging::init(LogLevel::Info, LogFormat::Plaintext);

    // Setup the application
    let app = App {
        home_dir: args.get_home_dir()?,
        config_file: args.get_config_file_path()?,
        genesis_file: args.get_genesis_file_path()?,
        private_key_file: args.get_priv_validator_key_file_path()?,
        start_height: Some(Height::new(1)), // We always start at height 1
    };

    cmd.run(&app, &args.get_home_dir()?)
        .map_err(|error| eyre!("Failed to run testnet command {:?}", error))
}
```

The `DumpWal` command is used to dump the contents of the WAL.

```rust
fn dump_wal(_args: &Args, cmd: &DumpWalCmd) -> Result<()> {
    // This is a drop guard responsible for flushing any remaining logs when the program terminates.
    // It must be assigned to a binding that is not _, as _ will result in the guard being dropped immediately.
    let _guard = logging::init(LogLevel::Info, LogFormat::Plaintext);

    cmd.run(ProtobufCodec)
        .map_err(|error| eyre!("Failed to run dump-wal command {:?}", error))
}
```

Finally, note that the `DistributedTestnet` command is not implemented as it is not relevant for this tutorial.

## Run a local testnet

Once provided with an implementation of the `init` and `testnet` commands, you will be able to run a local testnet.

For this, let's build the application and run the `testnet` command:

```
$ cargo build
$ cargo run -- testnet --nodes 3 --home nodes
```

This will create the configuration for 3 nodes in the `nodes` folder.
Feel free to inspect this folder and look at the generated files.

Now, in 3 different terminals, start each node with the following command.
Replace `NODE` with `1`, `2` and `3`.

```
$ cargo run -- start --home nodes/NODE
```

Et voila, we are now running a 3 nodes local testnet!

If the nodes are not started concurrently, you may see that it takes a little while until they synchronize between themselves and end up on the same round.
After that, consensus should start running normally and decide on values very quickly.

Alternatively, you can copy the [`spawn.bash`](/code/examples/channel/spawn.bash) script from the example app at the root of the project and spawn multiple nodes concurrently with:

```
$ bash spawn.bash --nodes 3 --home nodes
```

The logs for each node can then be found at `nodes/X/logs/node.log`.

Press `Ctrl+C` to stop all the nodes.
