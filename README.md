<h1 align="center">
<img src="./assets/banner.png" alt="Malachite" width="2400" align="center">
</h1>

<h4 align="center">
    Flexible BFT Consensus Engine in Rust<br/>
    Includes state-of-the-art library implementing the Tendermint consensus algorithm
</h4>

---

[![Build Status][build-image]][build-link]
[![Quint tests][quint-image]][quint-link]
[![MBT tests][mbt-test-image]][mbt-test-link]
[![Code coverage][coverage-image]][coverage-link]
[![Apache 2.0 Licensed][license-image]][license-link]
![Rust Stable][rustc-image]
![Rust 1.82+][rustc-version]
[![Quint 0.18][quint-version]][quint-repo]

[![Telegram Chat][tg-badge]][tg-url]

## About

Malachite is a Byzantine-fault tolerant (BFT) consensus library implemented in Rust. 
The goal is to enable developers to decentralize whatever the future may bring—sequencers, social networks, Layer 1s, etc.

Bundled with Malachite comes a state-of-the-art library implementing the Tendermint consensus algorithm.
Tendermint is a BFT consensus algorithm that is [optimistically responsive][responsive] and therefore
exhibits high-performance.
Additionally, this algorithm found adoption in many decentralized systems through its implementation in Go as part of [CometBFT](https://github.com/cometbft/cometbft/).
CometBFT has been battle-tested for years, and many of the lessons and experiences of maintaining CometBFT inspired key [design decisions][announcement] that we took in Malachite.

> [!IMPORTANT]
> Malachite is pre-alpha software and still under heavy development.
> At this stage, it is not meant for use in production.
> The software is provided "as is" and has not been externally audited, use at your own risk.

## Repository Overview

The repository is split in three areas, each covering one of the important areas of this project:

1. [code](./code): Comprises the Rust implementation of the Tendermint consensus algorithm, split across multiple Rust crates.
2. [docs](./docs): Comprises Architectural Decision Records (ADRs) and other documentation, such as the 2018 paper describing the core consensus algorithm.
3. [specs](./specs): English and [Quint][quint-repo] specifications.

## Requirements

- Rust v1.82+ ([rustup.rs](https://rustup.rs))
- Quint v0.18+ ([github.com](https://github.com/informalsystems/quint))

## Join Us

Malachite is developed by [Informal Systems](https://informal.systems).

If you'd like to work full-time on challenging problems of and distributed systems and decentralization,
[we're always looking for talented people to join](https://informal.systems/careers)!


## License

Copyright © 2024 Informal Systems Inc.

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
[tg-url]: https://t.me/MalachiteLibrary
[tg-badge]: https://img.shields.io/badge/Malachite-Library-blue.svg?logo=data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI+PHBhdGggZD0iTTEyIDI0YzYuNjI3IDAgMTItNS4zNzMgMTItMTJTMTguNjI3IDAgMTIgMCAwIDUuMzczIDAgMTJzNS4zNzMgMTIgMTIgMTJaIiBmaWxsPSJ1cmwoI2EpIi8+PHBhdGggZmlsbC1ydWxlPSJldmVub2RkIiBjbGlwLXJ1bGU9ImV2ZW5vZGQiIGQ9Ik01LjQyNSAxMS44NzFhNzk2LjQxNCA3OTYuNDE0IDAgMCAxIDYuOTk0LTMuMDE4YzMuMzI4LTEuMzg4IDQuMDI3LTEuNjI4IDQuNDc3LTEuNjM4LjEgMCAuMzIuMDIuNDcuMTQuMTIuMS4xNS4yMy4xNy4zMy4wMi4xLjA0LjMxLjAyLjQ3LS4xOCAxLjg5OC0uOTYgNi41MDQtMS4zNiA4LjYyMi0uMTcuOS0uNSAxLjE5OS0uODE5IDEuMjI5LS43LjA2LTEuMjI5LS40Ni0xLjg5OC0uOS0xLjA2LS42ODktMS42NDktMS4xMTktMi42NzgtMS43OTgtMS4xOS0uNzgtLjQyLTEuMjA5LjI2LTEuOTA4LjE4LS4xOCAzLjI0Ny0yLjk3OCAzLjMwNy0zLjIyOC4wMS0uMDMuMDEtLjE1LS4wNi0uMjEtLjA3LS4wNi0uMTctLjA0LS4yNS0uMDItLjExLjAyLTEuNzg4IDEuMTQtNS4wNTYgMy4zNDgtLjQ4LjMzLS45MDkuNDktMS4yOTkuNDgtLjQzLS4wMS0xLjI0OC0uMjQtMS44NjgtLjQ0LS43NS0uMjQtMS4zNDktLjM3LTEuMjk5LS43OS4wMy0uMjIuMzMtLjQ0Ljg5LS42NjlaIiBmaWxsPSIjZmZmIi8+PGRlZnM+PGxpbmVhckdyYWRpZW50IGlkPSJhIiB4MT0iMTEuOTkiIHkxPSIwIiB4Mj0iMTEuOTkiIHkyPSIyMy44MSIgZ3JhZGllbnRVbml0cz0idXNlclNwYWNlT25Vc2UiPjxzdG9wIHN0b3AtY29sb3I9IiMyQUFCRUUiLz48c3RvcCBvZmZzZXQ9IjEiIHN0b3AtY29sb3I9IiMyMjlFRDkiLz48L2xpbmVhckdyYWRpZW50PjwvZGVmcz48L3N2Zz4K
[responsive]: https://informal.systems/blog/tendermint-responsiveness
[announcement]: https://informal.systems/blog/malachite-decentralize-whatever