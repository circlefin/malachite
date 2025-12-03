# Contributing to Malachite

Thank you for your interest in contributing to Malachite, a Byzantine Fault Tolerant (BFT) consensus engine written in Rust. This guide explains how to set up your development environment and how to work with the codebase.

## Table of Contents

- [Scope of Contributions](#scope-of-contributions)
- [Setup](#setup)
  - [Prerequisites](#prerequisites)
  - [Environment Setup](#environment-setup)
- [Building the Project](#building-the-project)
- [Running Tests](#running-tests)
  - [Unit and Integration Tests](#unit-and-integration-tests)
  - [Model-Based Tests (MBT)](#model-based-tests-mbt)
- [Code Style and Guidelines](#code-style-and-guidelines)
- [Pull Request Process](#pull-request-process)
- [Continuous Integration](#continuous-integration)

## Scope of Contributions

Before contributing code, please read the “Scope of Contributions” section in [`CONTRIBUTING.md`](./CONTRIBUTING.md). That document explains which kinds of changes are currently accepted by the maintainers.

## Setup

### Prerequisites

To build and test Malachite you will need:

- **Rust** — installed via [rustup](https://rustup.rs/)
- **Node.js** — required for running [Quint](https://quint-lang.org) model-based tests
- **Quint** — installed as an npm package
- **cargo-nextest** — for running tests efficiently

### Environment Setup

1. **Install Rust** via `rustup`:

   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
