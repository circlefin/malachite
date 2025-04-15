# Contributing to Malachite

Thank you for your interest in contributing to Malachite, a Byzantine Fault Tolerant (BFT) consensus engine written in Rust.

This document provides guidelines and instructions to help you set up your development environment and contribute to the project.

## Table of Contents

- [Setup](#setup)
  - [Prerequisites](#prerequisites)
  - [Environment Setup](#environment-setup)
- [Building the Project](#building-the-project)
- [Running Tests](#running-tests)
  - [Unit Tests](#unit-tests)
  - [Integration Tests](#integration-tests)
  - [Model-Based Tests (MBT)](#model-based-tests-mbt)
- [Code Style and Guidelines](#code-style-and-guidelines)
- [Pull Request Process](#pull-request-process)
- [Continuous Integration](#continuous-integration)

## Setup

### Prerequisites

To build and test Malachite, you need the following tools:

- **Rust**: Install the latest stable Rust toolchain using [rustup](https://rustup.rs/)
- **Protocol Buffers Compiler (protoc)**: Required for Protobuf message serialization
- **Node.js**: Required for running [Quint](https://quint-lang.org)
- **Quint**: A formal specification language used for our model-based tests
- **cargo-nextest**: An improved test runner for Rust

### Environment Setup

1. **Install Rust**:

   Via [rustup.rs](https://rustup.rs):

   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **Install the Protocol Buffers Compiler**:

   `protoc` is needed for compiling Protobuf definitions used in the test applications to Rust code.

   For Ubuntu/Debian:

   ```bash
   sudo apt-get install protobuf-compiler
   ```

   For macOS:

   ```bash
   brew install protobuf
   ```

   Please ensure that the version of `protoc` is at least v29.0.

3. **Install Node.js**: (only required for running model-based tests)

   Follow the instructions at [nodejs.org](https://nodejs.org/) or use a version manager:

   ```bash
   # Using nvm
   curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.1/install.sh | bash
   nvm install 21
   nvm use 21
   ```

4. **Install Quint**: (only required for running model-based tests)

   ```bash
   npm install -g @informalsystems/quint
   ```

5. **Install cargo-nextest**:

   ```bash
   cargo install cargo-nextest
   ```

6. **Fork and clone the repository**:

   ```bash
   git clone https://github.com/USERNAME/malachite.git
   cd malachite/code
   ```

> [!NOTE]
> If you do not intend to contribute code and just want to play around with the codebase,
> you can just clone the repository directly: `git clone https://github.com/informalsystems/malachite.git`

## Building the Project

To build the project, run:

```bash
cargo build
```

## Running Tests

We have several categories of tests that help ensure the quality and correctness of our code.

### Unit and Integration Tests

```bash
cargo all-tests
```

Or run specific integration tests:

1. **Discovery**:

   ```bash
   cargo nextest run -p informalsystems-malachitebft-discovery-test
   ```

2. **Starknet app**:

   ```bash
   cargo nextest run -p informalsystems-malachitebft-starknet-test
   ```

3. **Test app**:

   ```bash
   cargo nextest run -p informalsystems-malachitebft-test
   ```

> [!IMPORTANT]
> For the integration tests to run successfully it is important to ensure that only one integration test is running at a time,
> by supplying the `--test-threads=1` flag to `cargo-nextest`. This is done automatically via the `code/.config/nextest.toml` configuration file,
> but can be overridden from the command line if needed.

> [!TIP]
> If you are on a Linux-based system, you can use [`cargo-maelstrom`](https://github.com/maelstrom-software/maelstrom) to run each test in isolation, concurrently.
>
> ```bash
> cargo install cargo-maelstrom
> cargo maelstrom --slots 16
> ```

### Model-Based Tests (MBT)

Model-based tests use formal specifications to generate test scenarios and validate system behavior against them.

To run model-based tests:

```bash
cargo mbt
```

## Code Style and Guidelines

We follow Rust's official style guidelines. Before submitting your code, please ensure:

1. **Format your code** using `rustfmt`:

   ```bash
   cargo fmt
   ```

2. **Run Clippy** to catch common mistakes and ensure code quality:

   ```bash
   cargo lint
   ```

## Pull Request Process

1. **Fork the repository** and create your branch from `main`.
2. **Make your changes** and ensure all tests pass.
3. **Update documentation** as needed.
4. **Submit a pull request** with a clear description of the changes and any relevant issue numbers.
5. **Address any feedback** from code reviewers.

When submitting a PR, our CI will automatically run all tests, Clippy checks, and formatting verification.

## Continuous Integration

We use GitHub Actions for continuous integration. The following checks run on every PR:

- **Unit Tests**: Ensures individual components work correctly.
- **Integration Tests**: Validates component interactions.
- **Model-Based Tests**: Checks against formal specifications.
- **Clippy**: Catches common Rust mistakes and enforces best practices.
- **Formatting**: Ensures consistent code style with rustfmt.

---

Thank you for contributing to Malachite! If you have any questions, feel free to open an issue or ask the maintainers.
