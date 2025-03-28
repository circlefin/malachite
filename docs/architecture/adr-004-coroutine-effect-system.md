# ADR 004: Coroutine-Based Effect System for Consensus

## Changelog

* 2025-03-18: Initial version
* 2025-03-27: Reviewed & accepted

## Status

Accepted

## Context

The Malachite core consensus implementation needs to interact with its environment (network, storage, cryptography, application logic) at specific points during execution.

> [!TIP]
> To understand the distinction between *core* and *non-core*, please see [ARCHITECTURE.md](../../ARCHITECTURE.md).

Traditional approaches to handling these interactions include:

1. **Callback-based designs:**
Define callbacks that the consensus implementation invokes when it needs external resources.
This model inverts control flow and makes the consensus code harder to follow, it is also not very idiomatic in the Rust community.

2. **Trait-based polymorphism:**
Define traits that users implement to provide required functionality, and have the consensus implementation call these methods.
This model enforces a specific execution model (sync/async) on the environment, which might not be desirable.

3. **Message-passing architectures:**
Define protocols for message-based communication between consensus and external components.
This model enforces a message-based architecture on the environment, which might not be desirable.

### Requirements

Instead of the above traditional approaches, we needed a design that would:

- Maintain a clear separation between the consensus algorithm and its environment
- Keep the consensus code linear and readable despite external interactions
- Support both synchronous and asynchronous operations (i.e., effects)
- Facilitate testing by making effects explicit and mockable
- Allow different execution environments (sync/async runtimes, actor systems, etc.)
- Handle errors gracefully without complicating the consensus core

## Decision

We've implemented a **coroutine-based effect system** that allows the core consensus algorithm to yield control when it needs external resources, and resume when the environment is ready to provide those resources. This design is different from the three traditional approaches we enumerated above.

### Key Components

1. **`Input` enum**: A type that represents all possible inputs that can be processed by the consensus coroutine.
2. **`Effect` enum**: A type that represents all possible interactions the consensus coroutine might need from its environment.
3. **`Resume` enum**: A type that represents all possible ways to resume the consensus coroutine after handling an effect.
4. **`Resumable` trait**: A trait that connects each effect with its corresponding `Resume` type.
5. **`process!` macro**: A macro that handles starting the coroutine, processing an input, yielding effects, and resuming consensus with appropriate values.

See [Appendix A](#appendix-a-details-of-the-coroutine-based-effect-system) for a detailed explanation of the underlying implementation of coroutine-based effect system using these components.

#### Input

The `Input` enum represents all possible inputs that can be processed by the consensus coroutine.

```rust
pub enum Input<Ctx>
where
    Ctx: Context,
{
    /// Start a new height with the given validator set.
    StartHeight(Ctx::Height, Ctx::ValidatorSet),

    /// Process a vote received over the network.
    Vote(SignedVote<Ctx>),

    /// Process a Proposal message received over the network
    ///
    /// This input MUST only be provided when `ValuePayload` is set to `ProposalOnly` or `ProposalAndParts`,
    /// i.e. when consensus runs in a mode where the proposer sends a Proposal consensus message over the network.
    Proposal(SignedProposal<Ctx>),

    /// Propose the given value.
    ///
    /// This input MUST only be provided when we are the proposer for the current round.
    Propose(LocallyProposedValue<Ctx>),

    /// A timeout has elapsed.
    TimeoutElapsed(Timeout),

    /// We have received the full proposal for the current round.
    ///
    /// The origin denotes whether the value was received via consensus gossip or via the sync protocol.
    ProposedValue(ProposedValue<Ctx>, ValueOrigin),

    /// We have received a commit certificate via the sync protocol.
    CommitCertificate(CommitCertificate<Ctx>),
}
```

#### Effect

The `Effect` enum represents all possible operations that the consensus coroutine might need from the environment.

```rust
pub enum Effect<Ctx>
where
    Ctx: Context,
{
    /// Reset all timeouts to their initial values
    ///
    /// Resume with: [`resume::Continue`]
    ResetTimeouts(resume::Continue),

    /// Cancel all outstanding timeouts
    ///
    /// Resume with: [`resume::Continue`]
    CancelAllTimeouts(resume::Continue),

    /// Cancel a given timeout
    ///
    /// Resume with: [`resume::Continue`]
    CancelTimeout(Timeout, resume::Continue),

    /// Schedule a timeout
    ///
    /// Resume with: [`resume::Continue`]
    ScheduleTimeout(Timeout, resume::Continue),

    /// Get the validator set at the given height
    ///
    /// Resume with: [`resume::ValidatorSet`]
    GetValidatorSet(Ctx::Height, resume::ValidatorSet),

    /// Consensus is starting a new round with the given proposer
    ///
    /// Resume with: [`resume::Continue`]
    StartRound(Ctx::Height, Round, Ctx::Address, resume::Continue),

    /// Publish a message to our peers
    ///
    /// Resume with: [`resume::Continue`]
    Publish(SignedConsensusMsg<Ctx>, resume::Continue),

    /// Rebroadcast our previous vote to our peers
    ///
    /// Resume with: [`resume::Continue`]
    Rebroadcast(SignedVote<Ctx>, resume::Continue),

    /// Requests the application to build a value for consensus to run on.
    ///
    /// Because this operation may be asynchronous, this effect does not expect a resumption
    /// with a value, rather the application is expected to propose a value within the timeout duration.
    ///
    /// The application SHOULD eventually feed a [`Propose`][Input::Propose]
    /// input to consensus within the specified timeout duration.
    ///
    /// Resume with: [`resume::Continue`]
    GetValue(Ctx::Height, Round, Timeout, resume::Continue),

    /// Requests the application to re-stream a proposal that it has already seen.
    ///
    /// The application MUST re-publish again to its peers all
    /// the proposal parts pertaining to that value.
    ///
    /// Resume with: [`resume::Continue`]
    RestreamProposal {
        /// Height of the value
        height: Ctx::Height,
        /// Round of the value
        round: Round,
        /// Valid round of the value
        valid_round: Round,
        /// Address of the proposer for that value
        proposer: Ctx::Address,
        /// Value ID of the value to restream
        value_id: ValueId<Ctx>,
        /// For resumption
        resume: resume::Continue,
    },

    /// Notifies the application that consensus has decided on a value.
    ///
    /// This message includes a commit certificate containing the ID of
    /// the value that was decided on, the height and round at which it was decided,
    /// and the aggregated signatures of the validators that committed to it.
    ///
    /// It also includes the vote extensions that were received for this height.
    ///
    /// Resume with: [`resume::Continue`]
    Decide(CommitCertificate<Ctx>, VoteExtensions<Ctx>, resume::Continue),

    /// Sign a vote with this node's private key
    ///
    /// Resume with: [`resume::SignedVote`]
    SignVote(Ctx::Vote, resume::SignedVote),

    /// Sign a proposal with this node's private key
    ///
    /// Resume with: [`resume::SignedProposal`]
    SignProposal(Ctx::Proposal, resume::SignedProposal),

    /// Verify a signature
    ///
    /// Resume with: [`resume::SignatureValidity`]
    VerifySignature(
        SignedMessage<Ctx, ConsensusMsg<Ctx>>,
        PublicKey<Ctx>,
        resume::SignatureValidity,
    ),

    /// Verify a commit certificate
    ///
    /// Resume with: [`resume::CertificateValidity`]
    VerifyCertificate(
        CommitCertificate<Ctx>,
        Ctx::ValidatorSet,
        ThresholdParams,
        resume::CertificateValidity,
    ),

    /// Allows the application to extend its precommit vote with arbitrary data.
    ///
    /// When consensus is preparing to send a pre-commit vote, it first calls `ExtendVote`.
    /// The application then returns a blob of data called a vote extension.
    /// This data is opaque to the consensus algorithm but can contain application-specific information.
    /// The proposer of the next block will receive all vote extensions along with the commit certificate.
    ///
    /// Only emitted if vote extensions are enabled.
    ///
    /// Resume with: [`resume::VoteExtension`]
    ExtendVote(Ctx::Height, Round, ValueId<Ctx>, resume::VoteExtension),

    /// Verify a vote extension.
    ///
    /// If the vote extension is deemed invalid, the vote it was part of will be discarded altogether.
    ///
    /// Only emitted if vote extensions are enabled.
    ///
    /// Resume with: [`resume::VoteExtensionValidity`]
    VerifyVoteExtension(
        Ctx::Height,
        Round,
        ValueId<Ctx>,
        SignedExtension<Ctx>,
        PublicKey<Ctx>,
        resume::VoteExtensionValidity,
    ),

    /// Consensus has been stuck in Prevote or Precommit step, and needs to ask for vote set from its peers
    /// in order to make progress. Part of the VoteSync protocol.
    ///
    /// Resume with: [`resume::Continue`]
    RequestVoteSet(Ctx::Height, Round, resume::Continue),

    /// A peer has requested a vote set from us, send them the response.
    /// Part of the VoteSync protocol.
    ///
    /// Resume with: [`resume::Continue`]`
    SendVoteSetResponse(RequestId, Ctx::Height, Round, VoteSet<Ctx>, Vec<PolkaCertificate<Ctx>>, resume::Continue),

    /// Append a consensus message to the Write-Ahead Log for crash recovery
    ///
    /// Resume with: [`resume::Continue`]`
    WalAppendMessage(SignedConsensusMsg<Ctx>, resume::Continue),

    /// Append a timeout to the Write-Ahead Log for crash recovery
    ///
    /// Resume with: [`resume::Continue`]`
    WalAppendTimeout(Timeout, resume::Continue),
}
```

#### Resume

The `Resume` enum represents all possible ways to resume the consensus coroutine after handling an effect.

Values of this type cannot be constructed directly, they can only be created by calling the `resume_with` method on a `Resumable` type.

```rust
pub enum Resume<Ctx>
where
    Ctx: Context,
{
    /// Resume execution without a value.
    Continue,

    /// Resume execution with `Some(Ctx::ValidatorSet)` if a validator set
    /// was successfully fetched, or `None` otherwise.
    ValidatorSet(Option<Ctx::ValidatorSet>),

    /// Resume execution with the validity of a signature
    SignatureValidity(bool),

    /// Resume execution with a signed vote
    SignedVote(SignedMessage<Ctx, Ctx::Vote>),

    /// Resume execution with a signed proposal
    SignedProposal(SignedMessage<Ctx, Ctx::Proposal>),

    /// Resume execution with the result of the verification of the [`CommitCertificate`]
    CertificateValidity(Result<(), CertificateError<Ctx>>),

    /// Resume with an optional vote extension.
    /// See the [`Effect::ExtendVote`] effect for more information.
    VoteExtension(Option<SignedExtension<Ctx>>),

    /// Resume execution with the result of the verification of the [`SignedExtension`]
    VoteExtensionValidity(Result<(), VoteExtensionError>),
}
```

#### Resumable

The `Resumable` trait allows creating a `Resume` value after having processed an effect.

```rust
pub trait Resumable<Ctx>
  where Ctx: Context
{
    /// The value type that will be used to resume execution
    type Value;

    /// Creates the appropriate [`Resume`] value to resume execution with.
    fn resume_with(self, value: Self::Value) -> Resume<Ctx>;
}
```

#### `process!`

The `process!` macro is the entry point for feeding inputs to consensus.
We omit its implementation here for brevity, but it handles starting the coroutine, processing an input, yielding effects, and resuming consensus with appropriate values.

One can think of it as a function with the following signature, depending on whether the effect handler is synchronous or asynchronous:

```rust
// If the effect handler is synchronous
fn process<Ctx>(
    input: Input<Ctx>,
    state: &mut ConsensusState<Ctx>,
    metrics: &Metrics,
    with: impl FnMut(Effect<Ctx>) -> Result<Resume<Ctx>, Error>,
) -> Result<(), ConsensusError<Ctx>>
where
    Ctx: Context;
```

```rust
// If the effect handler is asynchronous
async fn process<Ctx>(
    input: Input<Ctx>,
    state: &mut ConsensusState<Ctx>,
    metrics: &Metrics,
    with: impl AsyncFnMut(Effect<Ctx>) -> Result<Resume<Ctx>, Error>,
) -> Result<(), ConsensusError<Ctx>>
where
    Ctx: Context;
```

### Flow

1. The application calls `process!` with an input, consensus state, metrics, and an effect handler.
2. This initializes a coroutine which immediately starts processing the input.
3. The coroutine runs until it needs something from the environment.
4. At that point, the coroutine yields an `Effect` (like `SignVote` or `GetValue`).
5. The effect handler performs the requested operation.
6. For synchronous effects (like `SignVote`), the handler immediately resumes the coroutine with the result.
7. For asynchronous effects (like `GetValue`), the handler immediately resumes the coroutine with a `()` (unit) value,
   and will typically schedule a background task to provide the result later by feeding it as a new input back to consensus via the `process!` macro.

#### Possible effects yielded by an input

Here is a list of all effects that can be yielded when processing an input:

* StartHeight:
  - CancelAllTimeouts
  - ResetTimeouts
  - ScheduleTimeout
  - GetValue
  - Effects for any pending inputs

* Vote
  - WalAppendMessage
  - VerifyVoteExtension
  - GetValidatorSet
  - VerifySignature

* Proposal:
  - WalAppendMessage
  - GetValidatorSet
  - VerifySignature
  - Publish
  - Same as *Vote*

* Propose:
  - RestreamProposal
  - WalAppendMessage
  - GetValidatorSet
  - VerifySignature
  - Publish

* TimeoutElapsed;
  - WalAppendTimeout
  - Decide

* ProposedValue:
  - CancelTimeout
  - Same as *Vote*

* CommitCertificate:
  - GetValidatorSet
  - VerifyCertificate
  - Decide

* VoteSetRequest:
  - SendVoteSetResponse

* VoteSetResponse:
  - Same as *Vote*

## Consequences

### Positive

1. **Separation of concerns**: The consensus algorithm code remains focused on the state machine logic without environment dependencies.
2. **Code readability**: The consensus code retains a linear, procedural flow despite the need for external interactions.
3. **Flexibility**: The same consensus implementation can work in different execution environments (embedded systems, async runtimes, actor systems, etc.)
4. **Testability**: Effects are explicit and the effect handler can be easily mocked for testing.
5. **Error handling**: Clear points where environment errors can be handled without complicating the consensus core.

### Negative

1. **Learning curve**: The coroutine-based approach might be unfamiliar to some developers.
2. **Effect handling consistency**: Requires careful documentation and examples to ensure users handle effects correctly.
3. **Complexity with asynchronous effects**: The pattern for handling asynchronous effects like `GetValue` requires additional understanding.

## Implementation Notes

1. The coroutine implementation relies on the `genawaiter` crate, which allows defining functions which can yield values and be resumed later.
2. Each `Effect` variant carries a value that implements `Resumable`, which knows how to create the appropriate `Resume` variant.
3. The `process!` macro handles the boilerplate of creating the coroutine, handling effects, and resuming execution.
4. For asynchronous effects, the consensus must be resumed immediately, and the result must be provided later as a new input.
5. Error handling is done at the effect handler level, with fallback behaviors defined to allow consensus to continue even if an operation fails.

## Alternatives Considered

1. **Trait-Based Dependencies**: Requiring the caller to implement traits for all external functionality. Rejected because this would enforce either a synchronous or asynchronous execution environment. Traits in Rust currently cannot be agnostic to the execution model (sync vs async), so we would need either separate sync/async traits or commit to one model, limiting flexibility for integrators.
2. **Full Message Passing**: Making all interactions message-based. Rejected because it would lose the linear flow of the consensus algorithm, making it harder to understand and maintain.
3. **Futures/Promises**: Making all effects return futures. Rejected because it would tie the consensus core to a specific async runtime and force all integrations to use async Rust, even in environments where synchronous execution might be preferred.
4. **Thread-Per-Consensus-Instance**: Running each consensus instance in its own thread with blocking calls. Rejected due to performance and resource utilization concerns, especially for systems that need to run multiple consensus instances.
5. **Callback-Based API**: Providing callbacks for all external operations. Rejected because it would invert control flow and make the code harder to follow.

The coroutine-based approach offers the best balance of separation of concerns, code readability, and flexibility. It allows the consensus core to remain agnostic about sync versus async execution models, enabling integrators to choose the environment that best suits their needs while maintaining a consistent API.

## Example

This example demonstrates a comprehensive integration of Malachite within an asynchronous application architecture using Tokio.
It showcases how to handle both synchronous and asynchronous effects while maintaining a clean separation between the consensus algorithm and its environment.

The example implements a consensus node that:

- Listens for network events from peers
- Processes incoming consensus messages
- Handles consensus effects, including asynchronous value building
- Uses a message queue to feed back asynchronous results to the consensus engine

### Main loop

The `main` function establishes:
- A Tokio channel for queueing inputs to be processed by consensus
- A network service for receiving external messages
- The consensus state initialization with application-specific context

The main loop uses `tokio::select!` to concurrently handle two event sources:
1. Incoming network messages (votes, proposals, etc.)
2. Internally queued consensus inputs (like asynchronously produced values)

### Input processing

The `process_input` function serves as the entry point for all consensus inputs, whether from the network or internal queues. It:
- Takes the input and consensus state
- Invokes the `process!` macro to run the consensus algorithm
- Handles any effects yielded by the consensus algorithm using `handle_effect`

### Effect handling

The `handle_effect` function demonstrates handling both synchronous and asynchronous effects:

1. **Synchronous effects** (`SignVote`, `VerifySignature`):
   - Perform the operation immediately
   - Resume consensus with the result directly

2. **Asynchronous effects** (`GetValue`):
   - Resume consensus immediately without a result to allow it to continue
   - Spawn a background task to perform the longer-running operation
   - Queue the result as a new input to be processed by consensus later

3. **Network communication** (`Publish`):
   - Uses the network service to broadcast messages to peers
   - Waits for the operation to complete using `.await`

```rust
use std::sync::Arc;

use malachitebft_core_types::{Context, SignedVote};
use malachitebft_core_consensus::{
  process, Effect, Input, Resume, State as ConsensusState, Params as ConsensusParams
};

use myapp::{MyContext, Vote};

#[tokio::main]
async fn main() {
    let (tx_queue, rx_queue) = tokio::mpsc::channel(16);

    let network_service = NetworkService::new();

    let params = ConsensusParams::new(/* ... */);
    let mut state = ConsensusState::new(MyContext, params);

    tokio::select! {
        network_event = network_service.recv_msg() => {
            match network_event {
                NetworkEvent::Vote(vote) => {
                    process_input(Input::Vote(vote), &mut state, &metrics, &network_service, &tx_queue)
                }
                // ...
            }
        },

        input = rx_queue.recv() => {
            process_input(input, &mut state, &metrics, &network_service, &tx_queue)
        }
    }
}


// Function to process an input for consensus
pub async fn process_input(
   &mut self,
   input: Input<MyContext>,
   state: &mut ConsensusState<MyContext>,
   metrics: &Metrics,
   network_service: &NetworkService,
   input_queue: &Sender<Input<MyContext>>,
) -> Result<(), ConsensusError<MyContext> {
    // Call the process! macro with our external effect handler
    process!(
        input: input,
        state: state,
        metrics: metrics,
        with: effect => handle_effect(effect, network_service, input_queue)
    )
}

// Method for handling effects
async fn handle_effect(
    effect: Effect<MyContext>,
    network_service: &NetworkService,
    tx_queue: &Sender<Input<MyContext>>,
) -> Result<Resume<MyContext>, Error> {
    match effect {
        Effect::SignVote(vote, r) => {
            // Logic to sign a vote
            let signed_vote = sign_vote(vote);

            Ok(r.resume_with(signed_vote))
        },

        Effect::VerifySignature(msg, pk, r) => {
            // Logic to verify a signature
            let is_valid = verify_signature(&msg, &pk);

            Ok(r.resume_with(is_valid))
        },

        Effect::Publish(msg, r) => {
            // Logic to publish a message over the network
            network_service.publish(msg).await;

            Ok(r.resume_with(()))
        },

        Effect::GetValue(height, round, timeout, r) => {
            // Extract the timeout duration
            let timeout_duration = get_timeout_duration(timeout);

            // Spawn a task to build the value asynchronously
            let tx_queue = tx_queue.clone();
            tokio::spawn(async move {
                // Build the value (collecting txs, executing, etc.)
                let value = build_value(height, round, timeout_duration).await;

                // Put the `ProposeValue` consensus input in a queue,
                // for it to be processed by consensus at a later point.
                if let Ok(value) = result {
                    tx_queue.send(Input::ProposeValue(value));
                }
            });

            // Resume consensus immediately
            Ok(r.resume_with(()))
        }

        // Handle other effects...
    }
}
```

### Notes

#### Async/await

The example demonstrates how to integrate Malachite's effect system with Rust's async/await:
- The effect handler is an async function
- Network operations can be awaited
- Long-running operations can be spawned as background tasks

#### Input queue

The input queue (`tx_queue`/`rx_queue`) serves as a crucial mechanism for:
- Feeding asynchronously produced results back to consensus
- Ensuring consensus processes inputs sequentially, even when they're produced concurrently
- Decoupling background tasks from the consensus state machine

#### Effect handling

The `handle_effect` function shows:
- Clear pattern matching on different effect types
- Proper resumption with appropriate values
- Background task spawning for asynchronous operations
- Error handling for operations that might fail

#### Handling of the `GetValue` effect

The `GetValue` effect handling is particularly noteworthy:
1. It immediately resumes consensus with `()` (allowing consensus to continue)
2. It spawns a background task that:
   - Builds a value with a timeout
   - When complete, queues a `ProposeValue` input
3. The main loop will eventually receive this input from the queue and process it

This pattern allows consensus to make progress while waiting for potentially long-running operations like transaction execution and block construction.

#### Sync vs async boundary

The example elegantly handles the boundary between:
- The synchronous consensus algorithm (which yields effects and expects results)
- The asynchronous application environment (which processes effects using async operations)

This is achieved without requiring the consensus algorithm itself to be aware of async/await or any specific runtime.

## References

* See [Architecture.md](../../ARCHITECTURE.md) for an earlier and more naive introduction to the effect system design of the core consensus library.
* See [ADR 003](./adr-003-values-propagation.md) for more details on inputs `Proposal`, `Propose` and `ProposedValue`.

## Appendix A: Details of the coroutine-based effect system

Let's pretend that we are writing a program that needs to read a file from disk and then broadcast its contents over the network. We will call these operations _effects_.

If we were expressing this as an interface we might have a `Broadcast` trait:

```rust
trait Broadcast {
  async fn broadcast(s: String) -> Result<(), Error>;
}
```

and a `FileSystem` trait:

```rust
enum FileSystem {
  async fn read_file(path: PathBuf) -> Result<String, Error>;
}
```

And our program would look like:

```rust
async fn program(file: PathBuf, fs: impl FileSystem, b: impl Broadcast) -> Result<(), Error> {
  println!("Reading file from disk");
  let contents = fs.read_file(file).await?;

  println!("Broadcasting content");
  b.broadcast(contents).await?;

  Ok(())
}
```

The downside of this approach is that we are enforcing the use of async for all effects, which might not be desirable in all cases.
Moreover, we are introducing a trait for each effect, which might be cumbersome to maintain.
Alternatively, we could use a single trait with multiple methods, but this would make the trait less focused and harder to implement and mock, as we would have to implement all methods even if we only need one.

Instead, let's model our effects as data, and define an `Effect` enum with a variant per effect:

```rust
enum Effect {
  Broadcast(String),
  Read(PathBuf),
}
```

To model the return value of each effect, we define a `Resume` enum:

```rust
enum Resume {
  Broadcast(Result<(), Error>),
  ReadFile(Result<String, Error>),
}
```

Now, by defining an appropriate `perform!` macro, we can write a pure version of our program and choose how we want to interpret each effect later:

```rust
async fn program(
  co: Co<Effect, Resume>,
  file: PathBuf,
) -> Result<(), Error> {
  println!("Reading file from disk");

  let contents = perform!(Effect::ReadFile(file),
    Resume::FileContents(contents) => contents // contents has type `Result<String, Error>`
  ).await?;

  println!("Broadcasting content");

  perform!(Effect::Broadcast(contents),
    Resume::Sent(result) => result // `result` has type `Result<(), Error>`
  ).await?;

  Ok(())
}
```

The `perform!(effect, pat => expr)` macro yields an `effect` to be performed by the caller (handler) and eventually resumes the program with the value `expr` extracted by from the `Resume` value by the pattern `pat`.

We can now choose how we want interpret each of these effects when we run our program.

For instance, we could actually perform these effects against the network and the filesystem:

```rust
async fn perform_real(effect: Effect) -> Resume {
  match effect {
    Effect::ReadFile(path) => {
      let contents = tokio::fs::read_to_string(path).await;
      Resume::FileContents(contents)
    }
    Effect::Broadcast(data) => {
      let result = broadcast_over_network(data).await;
      Resume::Sent(result)
    }
  }
}

async fn main() {
  process!(
    program(_, "test.txt"),
    effect => perform_real(effect).await
  );
}
```

Or we can perform these effects against a mock file system and network, and for this we don't need to use async at all:

```rust
fn perform_mock(effect: Effect) -> Resume {
  match effect {
    Effect::ReadFile(path) => {
      Resume::FileContents("Hello, world")
    }
    Effect::Broadcast(data) => {
      Resume::Sent(Ok(()))
    }
  }
}

fn main() {
  process!(
    program(_, "test.txt"),
    effect => perform_mock(effect)
  );
}
```

Here we see one other advantage of modeling effects this way over using traits: **we can leave it up to the caller to decide whether or not to perform each effect in a sync or async context, instead of enforcing either with a trait (as methods in traits cannot be both sync and async).

The main drawback of this approach is that it is possible to resume the program using the wrong type of data:

```rust
fn perform_wrong(effect: Effect) -> Resume {
  match effect {
    Effect::ReadFile(path) => {
      Resume::Sent(Ok(())) // This should be `FileContents`, not `Sent`
    }
    Effect::Broadcast(data) => {
      Resume::FileContents(Ok("Hello, world".to_string())) // This should be `Sent`, not `FileContents`
    }
  }
}

fn main() {
  process!(
    program("test.txt"),
    effect => perform_wrong(effect)
  );
}
```

This program will crash at runtime with `UnexpectedResume` error telling us that the `ReadFile` effect expected to be resumed with `FileContents` and not `Sent`.

To mitigate this issue, we can define a `Resumable` trait that connects each effect with its corresponding `Resume` type:

```rust
trait Resumable {
    /// The value type that will be used to resume execution
    type Value;

    /// Creates the appropriate [`Resume`] value to resume execution with.
    fn resume_with(self, value: Self::Value) -> Resume;
}
```

We then define a new type per resume type and implement `Resumable` for each:

```rust
mod resume_with {
  struct Sent;

  impl Resumable for Sent {
      type Value = Result<(), Error>;

      fn resume_with(self, value: Self::Value) -> Resume {
          Resume::Sent(value)
      }
  }

  struct FileContents;

  impl Resumable for FileContents {
      type Value = Result<String, Error>;

      fn resume_with(self, value: Self::Value) -> Resume {
          Resume::FileContents(value)
      }
  }
}
```

We can now embed these types in each variant of the `Effect` enum:

```rust
enum Effect {
    Broadcast(String, resume_with::Sent),
    Read(PathBuf, resume_with::FileContents),
}
```

Note that these `resume_with` types are private and cannot be constructed directly, they can only be accessed by extracting them from an `Effect` variant.

In the effect handler, we can now use the `Resumable::resume_with` method to resume the program with the correct type:

```rust
fn perform_correct(effect: Effect) -> Resume {
    match effect {
        Effect::Read(path, r) => { // r is of type `resume_with::FileContents`
            let contents = tokio::fs::read_to_string(path).await;
            r.resume_with(contents) // returns a value of type `Resume::FileContents`
        }

        Effect::Broadcast(data, r) => { // r is of type `resume_with::Sent`
            let result = broadcast_over_network(data).await;
            r.resume_with(result) // returns a value of type `Resume::Sent`
        }
  }
}
```

We can now make the `Resume` type private so that it is impossible to construct it directly, and only the `Resumable` trait can be used to create it, effectively making it impossible to resume the program with the wrong type of data.

