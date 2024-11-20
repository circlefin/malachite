# Malachite

[![Build Status][build-image]][build-link]
[![Quint tests][quint-image]][quint-link]
[![MBT tests][mbt-test-image]][mbt-test-link]
[![Code coverage][coverage-image]][coverage-link]
[![Apache 2.0 Licensed][license-image]][license-link]
![Rust Stable][rustc-image]
![Rust 1.82+][rustc-version]
[![Quint 0.18][quint-version]][quint-repo]

Tendermint consensus in Rust

## Repository Overview

The repository is split in three areas, each covering one of the important areas of this project:

1. [code](./code): Comprises the Rust implementation of the Tendermint consensus algorithm, split across multiple Rust crates.
2. [docs](./docs): Comprises Architectural Decision Records (ADRs) and other documentation, such as the 2018 paper describing the core consensus algorithm.
3. [specs](./specs): English and [Quint][quint-repo] specifications.

## Requirements

- Rust v1.82+ ([rustup.rs](https://rustup.rs))
- Quint v0.18+ ([github.com](https://github.com/informalsystems/quint))

## License

Copyright © 2023 Informal Systems Inc.

Licensed under the Apache License, Version 2.0 (the "License"); you may not use the files in this repository except in compliance with the License. You may obtain a copy of the License at

    https://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software distributed under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied. See the License for the specific language governing permissions and limitations under the License.


[build-image]: https://github.com/informalsystems/malachite/actions/workflows/rust.yml/badge.svg
[build-link]: https://github.com/informalsystems/malachite/actions/workflows/rust.yml
[quint-image]: https://github.com/informalsystems/malachite/actions/workflows/quint.yml/badge.svg
[quint-link]: https://github.com/informalsystems/malachite/actions/workflows/quint.yml
[mbt-test-image]: https://github.com/informalsystems/malachite/actions/workflows/mbt.yml/badge.svg
[mbt-test-link]: https://github.com/informalsystems/malachite/actions/workflows/mbt.yml
[coverage-image]: https://codecov.informal.systems/gh/informalsystems/malachite/graph/badge.svg?token=LO0NSEJ9FC
[coverage-link]: https://codecov.informal.systems/gh/informalsystems/malachite
[license-image]: https://img.shields.io/badge/license-Apache_2.0-blue.svg
[license-link]: https://github.com/informalsystems/hermes/blob/master/LICENSE
[rustc-image]: https://img.shields.io/badge/Rust-stable-orange.svg
[rustc-version]: https://img.shields.io/badge/Rust-1.82+-orange.svg
[quint-version]: https://img.shields.io/badge/Quint-0.18-purple.svg
[quint-repo]: https://github.com/informalsystems/quint
