# Discovery

## Concept

A node has two connection parameters: outbound and inbound peers. Outbound peers are those that the node actively connects to; inbound peers are those that connect to the node. A peer can be outbound, inbound, or ephemeral—temporary connections used for tasks like Kademlia requests.

A node prioritizes fulfilling its outbound connections. To establish a persistent connection, it sends a connect request. The receiving node assigns the requester as outbound or inbound if there’s capacity; otherwise, it rejects the request.

Once registered as an outbound or inbound peer, a node can open multiple connections, up to the limit set by the max_connections_per_peer parameter.

When an outbound connection is dropped, the node attempts to upgrade an existing inbound peer or find a new one (e.g., via Kademlia).

For more, please take a look at Chapter 3 of this [thesis](https://bastienfaivre.com/files/master-thesis.pdf).
