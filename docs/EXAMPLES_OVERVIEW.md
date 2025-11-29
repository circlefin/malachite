# Examples overview

The `examples/` directory collects applications and utilities that demonstrate
how to embed Malachite as a consensus engine in different contexts.

This document gives a high-level overview of what you can expect to find in
the examples and how to approach them as a developer new to Malachite.

---

## 1. Purpose of the examples

The examples are intended to:

- show how to wire Malachite into a host application
- illustrate typical configuration patterns
- provide end-to-end scenarios that go beyond unit tests
- serve as a starting point for building your own applications

They are not meant to be production-ready deployments, but rather reference
implementations that highlight integration points.

---

## 2. Types of examples

Examples may include:

- **simple single-node demos** that focus on basic message flow
- **multi-node scenarios** that demonstrate networking and consensus progress
- **integration with external components**, such as execution layers or
  application-specific logic
- **utilities** that help inspect or debug certain aspects of the engine

The exact set of examples may evolve over time as the project grows.

---

## 3. How to explore an example

A typical way to explore an example is:

1. Pick an example under `examples/` that matches your area of interest.
2. Read its README or documentation (if present) to understand:
   - what the example is trying to demonstrate
   - which components it depends on
3. Inspect the source code to see:
   - how the Malachite engine is instantiated and configured
   - how messages flow between the host application and the engine
4. Run the example using the commands described in its documentation
   (this may involve `cargo run`, `docker compose`, or other tooling).

When in doubt, look for comments in the example code that explain its structure.

---

## 4. Running examples

Each example may have its own entry point and command-line arguments. Common
patterns include:

- `cargo run --example <name>` from the repository root or a specific crate
- dedicated binaries or scripts under `examples/` or `scripts/`
- Docker-based setups for multi-node scenarios

Before running examples:

- ensure your environment meets the project requirements (Rust 1.82+, Quint 0.22+)
- follow any additional instructions provided in the example's documentation

---

## 5. Using examples as a starting point

If you are building your own application on top of Malachite:

1. Identify an example that is closest to your target architecture.
2. Copy or adapt only the pieces you need:
   - configuration handling
   - engine integration
   - basic logging and metrics
3. Replace any hard-coded assumptions (ports, keys, network sizes) with your
   own configuration model.
4. Add tests around your integration points to catch regressions as the project
   evolves.

Examples are snapshots of how to use Malachite at a point in time; your
production setup will likely diverge, but the examples are a useful reference.

---

## 6. Contributing improvements

Improvements to the examples are welcome, including:

- clearer documentation and comments
- additional scenarios that cover new features
- fixes to keep examples aligned with the main codebase

When proposing changes to examples, please explain:

- what the example demonstrates
- how your change makes it easier to understand or more realistic
- any additional requirements (dependencies, configuration, etc.)

This helps maintainers keep the examples useful and approachable for new users.
