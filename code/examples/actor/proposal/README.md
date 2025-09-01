
# Example actor-based app

This is an example application using the actor based integration with malachite

## Run a local testnet

### Prerequisites

Before running the examples, make sure you have the required development environment setup as specified in this [Setup](../../../CONTRIBUTING_CODE.md#setup) section.

### Compile the app

```
cargo build
```

### Setup the testnet

Generate configuration and genesis for three nodes using the `testnet` command:

```
cargo run --bin actor-app-proposal -- testnet --nodes 3 --home nodes
```

This will create the configuration for three nodes in the `nodes` folder. Feel free to inspect this folder and look at the generated files.

### Spawn the nodes

```
bash ./examples/actor/proposal/spawn.bash --nodes 3 --home nodes
```

If successful, the logs for each node can then be found at `nodes/X/logs/node.log`.

```
tail -f nodes/0/logs/node.log
```

Check the metrics

For the block time:

```
curl -s localhost:29000/metrics | grep 'time_per_block_[sum|count]'
```

For the number of rounds per block:

```
curl -s localhost:29000/metrics | grep consensus_round
```

Press `Ctrl-C` to stop all the nodes.

