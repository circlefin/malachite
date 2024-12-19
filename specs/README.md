# Malachite Specifications

This directory holds specifications of various Malachite components or protocols.
It also covers specifications related to external dependencies, i.e., application-level concerns.
For example, Malachite comprises a library implementing the [Tendermint consensus algorithm][tendermint-arxiv] in Rust, which is specified below.
The specifications also cover networking, synchronization, and broader Starknet protocols.

- [Consensus algorithm and implementation](./consensus/README.md)
- [Network design and requirements](./network/README.md)
- [Synchronization protocols](./synchronization/README.md)
- [Starknet components](./starknet/README.md)

[tendermint-arxiv]: https://arxiv.org/abs/1807.04938
