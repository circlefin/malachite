# Malachite contributor onboarding

This document is a short orientation guide for new contributors who have just discovered the Malachite repository and want to understand where to start.

It complements, but does not replace, the main documentation in `README.md`, `ARCHITECTURE.md`, and the ADRs in `docs/`.

---

## 1. What is Malachite?

Malachite is a Byzantine-fault tolerant (BFT) consensus engine implemented in Rust. It provides a state-of-the-art implementation of the Tendermint consensus algorithm and is designed to be embedded as a library in larger systems.

Before contributing, it is helpful to read:

- the top-level `README.md` for a high-level overview and links
- `ARCHITECTURE.md` for a deeper explanation of the design and component boundaries

---

## 2. Repository layout (very high level)

The repository is roughly split into three main areas:

- `code/` – Rust crates that implement the core consensus algorithm and the surrounding engine components
- `docs/` – additional documentation, including Architectural Decision Records (ADRs) and background material
- `specs/` – English and Quint specifications that describe the intended behaviour of the system

There are also supporting directories such as:

- `.github/` – CI and automation configuration
- `scripts/` – helper scripts and tooling
- `terraform/` – infrastructure-related configuration

---

## 3. Where to start as a new contributor?

A few good first steps:

1. Read `README.md` and `ARCHITECTURE.md` to get a sense of the system design.
2. Browse the crates listed under the “Crates and Status” section in `README.md` to see which area of the codebase is most relevant to your interests.
3. Look at open issues tagged as good first issues or documentation-related improvements.

It is usually safer to start with:

- documentation improvements
- small refactors
- additional tests that do not change external behaviour

---

## 4. Code, specs, and tests

Malachite makes heavy use of:

- Rust crates for the implementation
- specifications (including Quint) to describe the protocol
- various kinds of tests (unit tests, model-based tests, etc.)

As you explore the code, it can be helpful to:

- keep the specs open in parallel when reading the implementation
- search for existing tests that exercise the code you want to modify
- follow existing patterns for logging, error handling and metrics

---

## 5. Contribution process (high level)

For details, always refer to `CONTRIBUTING.md` and any additional contribution guides in this repository. In general, you can expect to:

1. Fork the repository on GitHub.
2. Create a feature branch in your fork.
3. Make focused changes with clear commit messages.
4. Open a pull request against the main Malachite repository.
5. Address review feedback from maintainers.

Keeping changes small and well-scoped makes it easier for maintainers to review and merge your work.

---

## 6. Getting help

If you are unsure about where to contribute or how best to approach a change:

- check the issues and discussions in this repository
- look for links to community channels in the `README.md`
- consider opening a GitHub discussion or asking maintainers for guidance on a proposed change

Clear communication and small, incremental contributions are encouraged.
