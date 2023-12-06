# ADR 001: High Level Architecture for Tendermint Consensus Implementation in Rust

## Changelog
* 2023-10-27: First draft.

## Context

This ADR provides architecture and design recommendations for the implementation of the Tendermint consensus protocol in Rust. The implementation follows the article ["The latest gossip on BFT consensus"](#References) and the English and Quint specifications located in the [Specs](../../Specs) directory.

### Terminology

We use terminology in line with [prior art on Tendermint and BFT consensus](https://docs.cometbft.com/v0.34/introduction/). To recall briefly:
- The consensus implementation reaches a decision on a _value_, which is the primary output. This is done repeatedly, such that the system proceeds in _heights_, and each height produces a new _value_.
- To reach decision on a value in a given height, multiple _rounds_ may be necessary. The algorithm starts from _round 0_.
- The implementation relies on exchanges of _proposals_ and _votes_. Each _round_ is associated with a specific _proposer_ which has the role of proposing a value to be decided upon.

## Decision

### Repository Overview

The repository is split in three areas, each covering one of the important areas of this project:
1. [Code](../../Code): Comprises the Rust implementation of the Tendermint consensus algorithm, split across multiple Rust crates.
2. [Docs](../../Docs): Comprises Architectural Decision Records (ADRs) such as the present file and other documentation.
3. [Specs](../../Specs): English and Quint specifications.

TODO: We should consider renaming Code into something else.
TODO: Consider using lower-case naming of the top-level folders, e.g., `specs` instead of `Specs`.
TODO: Do we need to describe the code layout and Rust crates, or is the description of the implementation below enough?

### Overview of the Tendermint Consensus Implementation 

The consensus implementation consists of the following components:
- Builder and Proposer Modules
- Gossip Module
- Host System
- Consensus Driver
- Multiplexer
- Vote Keeper
- Round State Machine
  It interacts with the external environment via the Context trait, which is described in more detail below.

This specification describes the components used by the consensus algorithm and does not cover the Builder/Proposer and the Gossip moduels.


![Consensus SM Architecture Diagram](assets/sm_arch.jpeg)

The components of the consensus implementation as well as the associated abstractions are described in more detail below.

### Data Types & Abstractions

#### Context
TODO: This section is still under discussion.
The Tendermint consensus implementation will satisfy the `Context` interface, detailed below.
The data types used by the consensus algorithm are abstracted to allow for different implementations.

```rust
/// This trait allows to abstract over the various datatypes
/// that are used in the consensus engine.
pub trait Context
    where
        Self: Sized,
{
    type Address: Address;
    type Height: Height;
    type Proposal: Proposal<Self>;
    type Validator: Validator<Self>;
    type ValidatorSet: ValidatorSet<Self>;
    type Value: Value;
    type Vote: Vote<Self>;
    type SigningScheme: SigningScheme; // TODO: Do we need to support multiple signing schemes?

    /// Sign the given vote our private key.
    fn sign_vote(&self, vote: Self::Vote) -> SignedVote<Self>;

    /// Verify the given vote's signature using the given public key.
    /// TODO: Maybe move this as concrete methods in `SignedVote`?
    fn verify_signed_vote(
        &self,
        signed_vote: &SignedVote<Self>,
        public_key: &PublicKey<Self>,
    ) -> bool;

    /// Build a new proposal for the given value at the given height, round and POL round.
    fn new_proposal(
        height: Self::Height,
        round: Round,
        value: Self::Value,
        pol_round: Round,
    ) -> Self::Proposal;

    /// Build a new prevote vote by the validator with the given address,
    /// for the value identified by the given value id, at the given round.
    fn new_prevote(
        height: Self::Height,
        round: Round,
        value_id: Option<ValueId<Self>>,
        address: Self::Address,
    ) -> Self::Vote;

    /// Build a new precommit vote by the validator with the given address,
    /// for the value identified by the given value id, at the given round.
    fn new_precommit(
        height: Self::Height,
        round: Round,
        value_id: Option<ValueId<Self>>,
        address: Self::Address,
    ) -> Self::Vote;
}
```

Note:
- TBD: we should figure out where to put `broadcast_message(), start_timer()`
    - @romac: Likely outside of the `Driver`, so left up to the runtime which drives the driver.


#### Consensus Driver

##### Data Structures

The Consensus Driver is concerned with running the consensus algorithm for a single height, ie. it drives the state machine across multiple rounds.

It is therefore initialized with the height once and the instance is destroyed once a value for that height has been decided. Other parameters are required during initialization and operation as described below.

```rust
pub struct Driver<Ctx>
    where
        Ctx: Context,
{
    pub ctx: Ctx,
    pub proposer_selector: Box<dyn ProposerSelector<Ctx>>,

    pub address: Ctx::Address,
    pub validator_set: Ctx::ValidatorSet,

    pub votes: VoteKeeper<Ctx>,
    pub round_state: RoundState<Ctx>,
    pub proposals: Proposals<Ctx>,
}

```

##### Input Events (External APIs)

The Consensus Driver receives input events from the peer-to-peer layer and other external modules it interacts with. 

```rust
pub enum Input<Ctx>
    where
        Ctx: Context,
{
    /// Start a new round
    NewRound(Ctx::Height, Round),

    /// Propose a value for the given round
    ProposeValue(Round, Ctx::Value),

    /// Receive a proposal, of the given validity
    Proposal(Ctx::Proposal, Validity),

    /// Receive a signed vote
    Vote(SignedVote<Ctx>),

    /// Receive a timeout
    TimeoutElapsed(Timeout),
}

```
Notes:
- Round `0` is always started by an external module. Subsequent rounds are started by the driver when the Round State Machine indicates it via the `NewRound` message.
- A proposal event must include a proposal and a `valid` flag indicating if the proposal is valid. The proposal must be complete, i.e. it must contain a complete value or an identifier of the value (`id(v)`). If the value is sent by the proposer in multiple parts, it is the responsibility of the Builder/Proposal modules to collect and verify all the parts and the proposal message in order to create a complete proposal and the validity flag.
- `Vote` can be a `Prevote` or `Precommit` vote.
- The driver interacts with the host system to start timers and expects to receive timeout events for the timers that it started and have fired. The timeouts can be:
```
    Propose,
    Prevote,
    Precommit,
```

##### Operation

The Driver sends votes to the Multiplexer module. The Driver expects that, whenever the Muliplexer (via the Vote Keeper) observes any threshold of votes for the first time and based on its state, it returns the multiplexed event to the Driver.

The Driver sends the multiplexed events to the Round State Machine which, once it processes the Driver events, returns consensus-related messages back to the Driver. The Driver then processes these messages and sends them to the Gossip module, the consensus environment, the Host System, or in some cases processes them internally (e.g. `NewRound(round)` message).

Notes:
- Proposals and vote messages must be signed by the sender and validated by the receiver. Signer must be the proposer for `Proposal` and a validator for `Vote`.
  - The driver performs signature verification of the messages it receives from the consensus environment via methods provided by the Context (see `verify_signed_vote()`)
- On `StartRound(round)` event, the Driver must determine if it is the proposer for the given round. For this it needs access to a `validator_set.get_proposer(round)` method or similar.
- When building a proposal the driver will use the `get_value()` method of the Builder/ Proposer module to retrieve the value to propose. 

##### Output Messages

```rust
pub enum Output<Ctx>
    where
        Ctx: Context,
{
    /// Start a new round
    NewRound(Ctx::Height, Round),

    /// Broadcast a proposal
    Propose(Ctx::Proposal),

    /// Broadcast a vote for a value
    Vote(SignedVote<Ctx>),

    /// Decide on a value
    Decide(Round, Ctx::Value),

    /// Schedule a timeout
    ScheduleTimeout(Timeout),

    /// Ask for a value to propose and schedule a timeout
    GetValueAndScheduleTimeout(Round, Timeout),
}
```

### Driver Context

The driver is passed a instance of the `Context` trait which defines all the data types used by this instance of the consensus engine, and also provides synchronous, stateless methods for creating and signing votes.

### Driver Environment

The driver can make use of an environment (or Builder/Proposer module) to get a value to propose.
This environment is defined as an async interface to be implemented by the code downstream of the `Driver`.

TODO - updated with the new async interface:
```rust
#[async_trait]
pub trait Env<Ctx>
where
    Ctx: Context,
{
    /// Get the value to propose.
    async fn get_value(&self) -> Ctx::Value;
}
```
#### Multiplexer
The Multiplexer is responsible for multiplexing the input data and returning the appropriate event to the Round State Machine.

The table below describes the input to the Multiplexer and the output events to the Round State Machine.
The input data is:
- The step change from the Round State Machine.
- The output events from the Vote Keeper.
- Proposals and votes from the Driver.


| step changed to | vote keeperthreshold | proposal        | Multiplexed Input to Round SM   | new step  | algo condition | output                         |
|---------| -------------------- | --------------- |---------------------------------| --------- | -------------- | ------------------------------ |
| new(??) | -                    | -               | NewRound                        | propose   | L11            | …                              |
| any     | PrecommitValue(v)    | Proposal(v)     | PropAndPrecommitValue           | commit    | L49            | decide(v)                      |
| any     | PrecommitAny         | \*              | PrecommitAny                    | any (unchanged) | L47            | sch\_precommit\_timer          |
| propose | none                 | InvalidProposal | InvalidProposal                 | prevote   | L22, L26       | prevote\_nil                   |
| propose | none                 | Proposal        | Proposal                        | prevote   | L22, L24       | prevote(v)                     |
| propose | PolkaPrevious(v, vr) | InvalidProposal | InvalidProposalAndPolkaPrevious | prevote   | L28, L33       | prevote\_nil                   |
| propose | PolkaPrevious(v, vr) | Proposal(v,vr)  | ProposalAndPolkaPrevious        | prevote   | L28, L30       | prevote(v)                     |
| prevote | PolkaNil             | \*              | PolkaNil                        | precommit | L44            | precommit\_nil                 |
| prevote | PolkaValue(v)        | Proposal(v)     | ProposalAndPolkaCurrent         | precommit | L36, L37       | (set locked and valid)precommit(v) |
| prevote | PolkaAny             | \*              | PolkaAny                        | prevote   | L34            | prevote timer                  |
| precommit | PolkaValue(v)        | Proposal(v)     | ProposalAndPolkaCurrent         | precommit | L36, L42       | (set valid)                    |
                    |


#### Vote Keeper

##### Data Structures

The Vote Keeper is concerned with keeping track of the votes received and the thresholds of votes observed for each round.
To this end, it maintains some state per each round:

```rust
pub struct PerRound<Ctx>
    where
        Ctx: Context,
{
    votes: RoundVotes<Ctx::Address, ValueId<Ctx>>,
    addresses_weights: RoundWeights<Ctx::Address>,
    emitted_outputs: BTreeSet<Output<ValueId<Ctx>>>,
}
```

```rust
pub struct VoteKeeper<Ctx>
    where
        Ctx: Context,
{
    total_weight: Weight,
    threshold_params: ThresholdParams,
    per_round: BTreeMap<Round, PerRound<Ctx>>,
}

```

- The quorum and minimum correct validator thresholds are passed in as parameters during initialization. These are used for the different threshold calculations.
- The `validator_set` is used to detect equivocation; also to ensure that prevote and precommit messages from the same validator are not counted twice for the same round, e.g. in the case of the `honest_threshold` case (`f+1` in L55 in the BFT paper) for prevotes and precommits.

##### Input Events (Internal APIs)

The Vote Keeper receives votes from the Consensus Driver via:

```rust
pub fn apply_vote(
    &mut self,
    vote: Ctx::Vote,
    weight: Weight,
    current_round: Round,
) -> Option<Output<ValueId<Ctx>>> 
```

##### Operation

The Vote Keeper keeps track of the votes received for each round and the total weight of the votes. It returns any thresholds seen **for the first time**.

##### Output Messages

The Driver receives these output messages from the Vote Keeper.

```rust
pub enum Message<C>
where 
    C: Context
pub enum Output<Value> {
    PolkaAny,
    PolkaNil,
    PolkaValue(Value),
    PrecommitAny,
    PrecommitValue(Value),
    SkipRound(Round),
}
```

#### Round State Machine

##### Data Structures

The Consensus State Machine is concerned with the internal state of the consensus algorithm for a given round. It is initialized with the height and round. When moving to a new round, the driver creates a new round state machine while retaining information from previous round (e.g. valid and locked values).

```rust
pub struct State<Ctx>
    where
        Ctx: Context,
{
    pub height: Ctx::Height,
    pub round: Round,

    pub step: Step,
    pub locked: Option<RoundValue<Ctx::Value>>,
    pub valid: Option<RoundValue<Ctx::Value>>,
}
```

##### Input Events (Internal APIs)

The Round state machine receives events from the Consensus Driver via:

```rust
pub fn apply(self, data: &Info<Ctx>, input: Input<Ctx>) -> Transition<Ctx> {
```

The events passed to the Round state machine are very close to the preconditions for the transition functions in the BFT paper, i.e., the `upon` clauses.
In addition:
- The `StartRound` events specify if the SM runs in the proposer mode or not. In the former case, the driver also passes a valid value to the round SM.
- There are two `Poposal` events, for valid and invalid values respectively. Therefore, the `valid(v)` check is not performed in the round SM but by the Driver

```rust
pub enum Input<Ctx>
    where
        Ctx: Context,
{
    /// Start a new round, either as proposer or not.
    /// L14/L20
    NewRound,

    /// Propose a value.
    /// L14
    ProposeValue(Ctx::Value),

    /// Receive a proposal.
    /// L22 + L23 (valid)
    Proposal(Ctx::Proposal),

    /// Receive an invalid proposal.
    /// L26 + L32 (invalid)
    InvalidProposal,

    /// Received a proposal and a polka value from a previous round.
    /// L28 + L29 (valid)
    ProposalAndPolkaPrevious(Ctx::Proposal),

    /// Received a proposal and a polka value from a previous round.
    /// L28 + L29 (invalid)
    InvalidProposalAndPolkaPrevious(Ctx::Proposal),

    /// Receive +2/3 prevotes for a value.
    /// L44
    PolkaValue(ValueId<Ctx>),

    /// Receive +2/3 prevotes for anything.
    /// L34
    PolkaAny,

    /// Receive +2/3 prevotes for nil.
    /// L44
    PolkaNil,

    /// Receive +2/3 prevotes for a value in current round.
    /// L36
    ProposalAndPolkaCurrent(Ctx::Proposal),

    /// Receive +2/3 precommits for anything.
    /// L47
    PrecommitAny,

    /// Receive +2/3 precommits for a value.
    /// L49
    ProposalAndPrecommitValue(Ctx::Proposal),

    /// Receive +1/3 messages from a higher round. OneCorrectProcessInHigherRound.
    /// L55
    SkipRound(Round),

    /// Timeout waiting for proposal.
    /// L57
    TimeoutPropose,

    /// Timeout waiting for prevotes.
    /// L61
    TimeoutPrevote,

    /// Timeout waiting for precommits.
    /// L65
    TimeoutPrecommit,
}
```

##### Operation

The Round State Machine keeps track of the internal state of consensus for a given round. It resembles very closely the algorithm description in the [original "The Latest gossip on BFT consensus" paper](#References).

##### Output Messages

The Round state machine returns the following messages to the Driver:

```rust
pub enum Output<Ctx>
    where
        Ctx: Context,
{
    NewRound(Round),                            // Move to the new round.
Proposal(Ctx::Proposal),                    // Broadcast the proposal.
Vote(Ctx::Vote),                            // Broadcast the vote.
ScheduleTimeout(Timeout),                   // Schedule the timeout.
GetValueAndScheduleTimeout(Round, Timeout), // Ask for a value and schedule a timeout.
Decision(RoundValue<Ctx::Value>),           // Decide the value.
}
```

## Status

Accepted

## Consequences

### Positive

- The abstraction offered by `enum Event` encapsulates all the complexity of `upon` clauses, it simplifies reasoning about the pure state machine logic within the Round State Machine.
- The semantics of counting votes and reasoning about thresholds is grouped into the Vote Keeper module and clearly separates that concern from the state machine logic.
- Functionality is offloaded to the host system wherever possible: The concerns of scheduling, managing, and firing timeouts.
- All sources of non-determinism have been excluded outside the boundaries of the consensus implementation, e.g. `valid` method, timeouts, I/O triggers, thus simplifying testing and reasoning about this system. 
- TODO: Events vs. Messages positive consequences.

### Negative

- The `enum Event` has numerous variants and comprises many nuances, thus may be difficult to understand.

### Neutral

- The concept of `Vote` is borrowed from earlier implementations of Tendermint consensus algorithm and this may be at times ambiguous.
- The concept of `Height` is borrowed from ["The latest gossip.."](#references) article and it may be inaccurate in some contexts. For example, a height is a straightforward concept in the context of implementing a blockchain system, but it may be inappropriate in the implementation of general-purpose sequencing systems, total-order logs, or atomic broadcast.  

## References

* [CometBFT v0.34 docs](https://docs.cometbft.com/v0.34/introduction/)
* ["The latest gossip on BFT consensus"](https://arxiv.org/pdf/1807.04938.pdf), by _Buchman, Kwon, Milosevic_. 2018.
