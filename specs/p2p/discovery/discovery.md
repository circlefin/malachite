# Iterative Peer Discovery (IPD) algorithm

The purpose of the algorithm is to help validator nodes find each other in the network starting from an initial set of nodes called **bootstrap nodes**. Especially, the algorithm is made so that a joining node does not need to know the whole network to join it.

Moreover, the same algorithm can be used to bootstrap a network from scratch (i.e. starting an initial set of nodes at the same time).

## Properties

- **Discoverability**: the joining nodes eventually discover all honest alive nodes.
- **Byzantine-resilience**: the algorithm is Byzantine-resilient.
- **Termination**: the algorithm eventually terminates with at least one honest alive node discovered.

## Assumptions

We assume that the new node is joining a network with the following properties:

- An honest node behaves as expected at all times.
- A node diverging at least once from the expected behavior is considered byzantine. This includes crashed nodes.

And the following properties on the network:

- **Connectivity**: an honest node can reach all other honest nodes via a path of honest nodes only at all times. This implies:
    - There are no isolated honest nodes or partitions in the network.
    - An honest node has at least one honest alive node.
- **Safe bootstrap**: there is at least one honest alive node in the set of bootstrap nodes of a node.
- **Byzantine**: there are at most `f` Byzantine nodes in the network.

## Algorithm

For clarity, we define the following request-response module interface:

```python
# Requests that can be triggered
event <sendRequest, node>               # Send a peer request to `node`
event <sendResponse, node, peers>       # Send the `peers` to `node`

# Indication to which we define callbacks
event <receivedRequest, node>           # Received a peer request from `node`
event <receivedResponse, node, peers>   # Received `peers` from `node`
event <noPendingRequest>                # Emitted when there are no more pending requests.
                                        #     We assume that this event is only emitted after
                                        #     some requests have occurred and all received
                                        #     responses have been processed.
```

And the discover interface:

```python
# Request that can be triggered
event <start, S>    # Initiate the algorithm with a set `S` of bootstrap nodes

# Indication to which we define callbacks
event <done, peers> # The discovery algorithm completed and found `peers`
```

Here is the algorithm:

```python
# Node's local variables
local_peers = {}
contacted = {}
bootstrap_nodes = {}

def contact(peers):
    for peer in peers:
        if peer not in contacted:
            trigger <sendRequest, peer, local_peers U bootstrap_nodes>
            add peer to contacted

upon event <start, S>: 
    bootstrap_nodes = S
    contact(bootstrap_nodes)

upon event <receivedRequest, node, peers>:
    # The difference is an optimization to not respond to the requesting
    # node with peers it already knows.
    # U => union, - => difference
    trigger <sendResponse, node, (local_peers U bootstrap_nodes) - peers >
    add node to local_peers
    contact(peers)
        
upon event <receivedResponse, node, peers>:
    add node to local_peers
    contact(peers)
                        
upon event <noPendingRequest>:
    trigger <done, local_peers>
```

The algorithm follows an iterative approach based on BFS. The idea is to request to an initial set of nodes (bootstrap nodes) the list of their known peers. Then, the node repeats the process with the new peers it discovered. The algorithm terminates when no new peers are discovered.
