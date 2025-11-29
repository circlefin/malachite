# Continuous integration (CI) pipelines overview

This document gives a high-level overview of the CI pipelines used in the
Malachite repository and how they relate to the different kinds of tests and
checks described in the README and project documentation.

It is intended for contributors who want to understand what will run on their
pull requests and how to reproduce these checks locally.

---

## 1. Goals of the CI setup

The CI pipelines aim to ensure that:

- the Rust code builds and passes tests on stable toolchains
- Quint-based specifications remain consistent and executable
- model-based tests (MBT) keep exercising important protocol scenarios
- code coverage remains tracked over time
- changes from contributors are validated in a repeatable environment

CI should provide fast feedback for everyday development, and deeper checks
for changes that affect core consensus logic.

---

## 2. Rust build and test workflows

Rust-focused workflows typically:

- build the workspace (or selected crates) in debug or release mode
- run unit and integration tests via `cargo test`
- may apply additional flags or features where appropriate

As a contributor, you can usually approximate these workflows with:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features
cargo test --workspace
