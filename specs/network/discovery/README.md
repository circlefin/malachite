# Peer Discovery

We assume that nodes participating in the network do not have an _a priori_
knowledge of the network composition.
Instead, nodes are initialized with the identities and addresses of a subset of
nodes in the network, to which they establish an initial set of connections,
and from which they expect to retrieve identities and addresses of other nodes
participating in the network.

Designed protocols:

- [Iterative Peer Discovery (IPD)](./ipd-protocol.md)
