# ADR 004: Coroutine-Based Effect System for Consensus

## Changelog

* 18-03-2025: Initial version

## Status

Accepted

## Context

The Malachite core consensus implementation needs to interact with its environment (network, storage, cryptography, application logic) at specific points during execution.

Traditional approaches to handling these interactions include:

1. **Callback-based designs:** Provide callback interfaces that must be implemented by users
2. **Trait-based polymorphism:** Define traits that users implement to provide required functionality
3. **Message-passing architectures:** Define protocols for communication between consensus and external components

We needed a design that would:
- Maintain a clear separation between the consensus algorithm and its environment
- Keep the consensus code linear and readable despite external interactions
- Support both synchronous and asynchronous operations
- Facilitate testing by making effects explicit and mockable
- Allow different execution environments (sync/async runtimes, actor systems, etc.)
- Handle errors gracefully without complicating the consensus core

## Decision

We've implemented a **coroutine-based effect system** using the `process!` macro that allows the consensus algorithm to yield control when it needs external resources, and resume when those resources are provided.

### Key Components

1. **`Input` enum**: A type that represents all possible inputs that can be processed by the consensus coroutine.
2. **`Effect` enum**: A type that represents all possible interactions the consensus coroutine might need from its environment.
3. **`Resume` enum**: A type that represents all possible ways to resume the consensus coroutine after handling an effect.
4. **`Resumable` trait**: A trait that connects each effect with its corresponding `Resume` type.
5. **`process!` macro**: A macro that handles starting the coroutine, processing an input, yielding effects, and resuming consensus with appropriate values.

### Flow

1. The application calls `process!` with an input, consensus state, metrics, and an effect handler.
2. This initializes a coroutine which immediately starts processing the input.
3. The coroutine runs until it needs something from the environment.
4. At that point, the coroutine yields an `Effect` (like `SignVote` or `GetValue`).
5. The effect handler performs the requested operation.
6. For synchronous effects (like `SignVote`), the handler immediately resumes the coroutine with the result.
7. For asynchronous effects (like `GetValue`), the handler immediately resumes the coroutine with a `()` (unit) value,
   and will typically schedule a background task to provide the result later by feeding it as a new input back to consensus via the `process!` macro.

## Consequences

### Positive

1. **Separation of concerns**: The consensus algorithm code remains focused on the state machine logic without environment dependencies.
2. **Code readability**: The consensus code retains a linear, procedural flow despite the need for external interactions.
3. **Flexibility**: The same consensus core can work in different execution environments (async runtimes, actor systems, etc.)
4. **Testability**: Effects are explicit and can be easily mocked for testing.
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

This example demonstrates a comprehensive integration of MalachiteBFT within an asynchronous application architecture using Tokio.
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
   - Resume consensus immediately with `()` to allow it to continue
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
            process_input(input, &mut state, &metrics, &tx_queue)
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
        with: effect => handle_effect(effect, input_queue)
    )
}

// Method for handling effects
async fn handle_effect(
    effect: Effect<MyContext>
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

> Are there any relevant PR comments, issues that led up to this, or articles referenced for why we made the given design choice? If so link them here!

* {reference link}

