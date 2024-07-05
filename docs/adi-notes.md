# Raw notes and thoughts traversing the code

adi seredinschi
june 28 - july 5

## Discussion topics with Anca & Greg

* Where does workload generation start from?
    * Seems like `generate_and_broadcast_txes`

* What is the ABCI equivalent for Malachite?
    * Seems like `trait Host`

* Why is `Node` actor supervising the `Host` & `Starknet Context`?  (@Romain's slides)
    * The `Node` is a monitor-like agent
    * Like Comet-style Node also, starts all reactors
    * StarknetApp -> Supervisor -> all other actors
    * Needed from the `ractor` library
    * But not necessary, actually
    * Someone else who is integrating can use just the library
    * Note that this is simpler/better than Comet!
    * It must be _actor_ based model, not go-routines
        * We just message-based state machines
        * We should impose the actor model, but not necessarily the specific library `ractor`

* Seems like the `Node` is a supervisor, is this the right mental model?
    * Consider making the `Node` abstraction external to Malachite -- it's an example of how the consensus core is being used in a practical, realistic setup

* Starknet app vs. Starknet host vs. Context
    * Recommend looking into the test-app
    * Found it confusing that the Starknet App does not have anything app-specific
        * Instead, most of the app-specific functionality seems delegated to `MockHost` (e.g., `build_new_proposal`) and generally `StarknetHost`
    * Took me quite a long time to realize I did not need to create my own `KvHost`, and instead I could just reuse the `MockHost`

* Does this implementation assume the delayed execution model?
    * Covered

* KV-store idea
    * Likely base it on the deleted test-app, which is also simpler..

* What is Malachite?
    * at minimum: a consensus library
    * at most: combination of node, consensus, and gossip consensus
        * the rest is contextual parts

* The code is very "flat" and a bit tough to navigate
    * Developer experience improvement by making it clearer what are the relevant points of configuration and customization of an application built using Malachite
    * Make it simpler to navigate to the important parts, to find e.g., where we consume blocks, where we produce them, and so on

## Bike-shedding topics

* Crates hierarchy, following Daniel's idea
    * consensus could include the following
        * driver
        * round
        * vote
        * ? anything else?

## Lower priority ideas if we have too much time to discuss

* Malachite = "sequencing kernel" in the same sense as Unix is a kernel
* rename consensus -> tendermint (unless it's a generic non-tendermint consensus type)
* `validator`: not widely used/common in the sequencer domain
    * maybe use `staker` or `sequencer` is better?
    * if we want to be more general, it could be a `replica`
* Update readme.md with above, maybe make a new ADR
    * Clarify that Malachite includes testing and integration code

## Glossary

### Node

- Malachite node = ??

### Application

- the use-case = decentralized sequencer
- this is the environment to Malachite

## What we have so far in terms of terminology

From ADR 001:

- The consensus implementation reaches a decision on a _value_, which is the primary output. This is done repeatedly, such that the system proceeds in _heights_, and each height produces a new _value_.
- To reach decision on a value in a given height, multiple _rounds_ may be necessary. The algorithm starts from _round 0_.
- The implementation relies on exchanges of _proposals_ and _votes_. Each _round_ is associated with a specific _proposer_ which has the role of proposing a value to be decided upon.