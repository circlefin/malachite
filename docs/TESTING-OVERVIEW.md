# Testing overview

This document gives a high-level overview of the testing strategy used in the Malachite project and how the different kinds of tests relate to each other.

It is meant as a conceptual guide and does not replace the more detailed instructions in existing contribution and testing documents.

---

## 1. Why so many types of tests?

Malachite implements a Byzantine-fault tolerant consensus engine, which is both safety-critical and complex. As a result, the project relies on several complementary approaches to testing and verification:

- conventional Rust tests
- model-based testing
- specification-based testing with Quint
- property-based and fuzz-style tests in selected components

Each style covers a different aspect of correctness and helps catch different classes of bugs.

---

## 2. Conventional Rust tests

Many crates under the `code/` directory contain regular Rust tests:

- unit tests that exercise individual functions or small modules
- integration-style tests that involve multiple components

These tests are typically run using the standard Cargo tooling (for example, via `cargo test` or the equivalent commands described in the contribution guides).

They are well-suited for:

- checking local invariants and error handling
- guarding specific regression cases
- keeping the development feedback loop fast

---

## 3. Model-based and specification-based tests

Malachite also relies on specifications and model-based testing to validate the behaviour of the consensus protocol.

At a high level:

- English and Quint specifications under `specs/` describe the intended behaviour of the protocol and key invariants.
- model-based testing frameworks can compare the implementation against a reference model derived from these specifications.

This approach is particularly valuable for:

- validating safety and liveness properties of the consensus algorithm
- exploring corner cases that are hard to reach with ad-hoc tests
- increasing confidence in the core protocol logic

For details on how to run these tests, refer to the existing documentation and contribution guides in the repository.

---

## 4. End-to-end and scenario tests

Some tests exercise larger parts of the system end to end, for example:

- multiple nodes exchanging messages
- chain progress under different network conditions
- failure and recovery scenarios

These tests help ensure that:

- the wiring between crates and components is correct
- the engine behaves as expected when used as a library in a larger application
- protocol-level guarantees hold in realistic settings, not only in unit-level code

They are typically more expensive to run, but provide valuable coverage for integration issues.

---

## 5. How to approach testing when contributing

When adding or changing code, it is often useful to:

1. Start by looking for existing tests around the code you are modifying.
2. Mirror the style and patterns you see there (naming, structure, helper functions).
3. Add:

   - a focused unit test for the specific behaviour you are changing, and
   - where appropriate, an integration or scenario test that exercises the change in a larger context.

If your change affects protocol semantics or invariants, consider whether:

- the relevant specification should be updated, and
- there should be a corresponding adjustment in the model-based or specification-based tests.

---

## 6. Where to find more information

For concrete commands and detailed instructions, please refer to:

- `CONTRIBUTING.md` and any dedicated testing sections or scripts
- documentation under `docs/` and `specs/`
- references to testing in `ARCHITECTURE.md` and crate-level READMEs

If you are unsure whether your test coverage is sufficient, it is always fine to ask for guidance in a pull request or in the project’s communication channels.
