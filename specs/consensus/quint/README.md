# Malachite Quint Specification

This specification is separated into 
- a functional layer (capturing the code), 
- a state machine layer (capturing the execution of Tendermint consensus in a distributed setting)
    - for simulation
    - for generation random traces (for model-based testing)
- runs / tests: that serve as 
    - documentation for interesting scenarios and 
    - tests for the functional layer

## Functional layer

- [Consensus logic](./consensus.qnt)
- [Vote keeper](./votekeeper.qnt)
- [Driver](./driver.qnt)

## State machine

- [State machine](./statemachineAsync.qnt)

## Runs

- [A domain-specific language](./TendermintDSL.qnt) to compose runs
- Files in the [`tests/`](./tests/) folder contain tests for different scenarios and parts of the specification 
