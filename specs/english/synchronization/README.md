# Malachite Synchronization Protocol Specification

We consider a composition of three components
- Cons. The consensus node: Executing consensus iteratively over multiple heights, and storing the decided blocks
- Client. Synchronization Client: That tries to obtain data (certificates, blocks) in order to decide quickly, in case the node has fallen behind, and other nodes have already decided on blocks.
- Server. Synchronization Server. The provides data about decided blocks to clients

#### Outline of the protocol