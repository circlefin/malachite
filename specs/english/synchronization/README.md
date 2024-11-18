# Malachite Synchronization Protocol MVP Specification

The challenge we address with the synchronization protocol is long-term stability: 
When running for a long time, the consensus mechanism alone may not be enough to keep the system alive. 
If nodes are down or disconnected for some (extended period of) time, they may fall behind several heights. 
While to consensus algorithm is fault-tolerant, if too many nodes fall behind, and are not able to catch up, eventually we might not have sufficiently many validators synchronized at the top of the chain to make progress.


We consider a composition of three components
- Consensus. The consensus node: Executing consensus iteratively over multiple heights, and storing the decided blocks
- Client. Synchronization Client: That tries to obtain data (certificates, blocks) in order to decide quickly, in case the node has fallen behind, and other nodes have already decided on blocks.
- Server. Synchronization Server. Provides data about decided blocks to clients

#### Outline of the protocol


The rough idea of the protocol is the following
- Consensus, client and server run in parallel on a node
- The client observes the height of its local consensus instance
- The server regularly announces the height of its local consensus instance to the network
- When a client observes that the local height is smaller than a remote height, it requests for a missing height: the commit (a certificate consisting of 2f+1 precommit messages) and the committed block (proposal)
- When a server receives such a request, it obtains the required information from the local block store, and sends it to the client
- When a client receives a response (certificate or proposal), it delivers this information to the consensus logic
- The consensus logic (driver) handles this incoming information in the same way as it would handle it if it came from "normal" consensus operation.

## Design decision

Observe that in the protocol description above, we have already taken a big design decision, namely, that the components consensus, client, and server run in parallel. This has been a conscious decision.

There are other potential designs, where the protocols alternate: when a node figures out that it has fallen behind several heights, it switches from consensus mode to synchronization mode, and then, when it has finished synchronizing, it switches back to consensus. This approach has two downsides (1) consensus blocks are decided in (at least) two locations of the code, consensus and synchronization, and (2) what are the precise conditions to switch between the two modes, and what are the expectations about the local state (e.g., incoming message buffers, etc.) when such a switch happens? Particularly Point 2 is quite hard to get right in a distributed setting. For instance, a node might locally believe that it is synchronized, while others have actually moved ahead instead. This creates a lot of algorithmic complexity as well as complications in the analysis of the protocols.

We have thus decided to go for another approach:

- Consensus, client and server run in parallel on a node (we don't need to define switching conditions between consensus and synchronization as they are always running together)
- the consensus logic is the single point where decisions are made (i.e., blocks are committed)
- the synchronization protocol is just an alternative source for certificates and proposals
- the synchronization can be run as add-on, and doesn't need any change to the consensus mechanism/architecture already implemented/specified in Malachite.
- Coupling of synchronization client and server to the consensus logic:
    - the server needs read access to the block store in order to retrieve certificates and blocks for committed heights
    - the client needs write access to incoming buffers (for certificates and blocks) of the consensus logic


## Central aspects of Synchronization

### Synchronization Messages

#### Status message
In regular intervals, each server sends out a status message, informing the others of its address and telling it from which height (base) to which height (top) it can provide certificates and blocks:
```bluespec
type StatusMsg = {
    peer: Address,
    base: Height,    
    top: Height
}
```

#### Request message
A client asks a specific peer either for a certificate or for a block at a given height:
```bluespec
type ReqType =
    | SyncCertificate
    | SyncBlock

type RequestMsg = {
    client: Address,
    server: Address,
    rtype: ReqType,
    height: Height
}
```

#### Response message
A server provides the required information to a client:
```bluespec
type Response = 
    | RespBlock(Proposal)
    | RespCertificate(Set[Vote])

type ResponseMsg = {
    client: Address,
    server: Address,
    height: Height,
    response: Response,
}
```

### Synchronization Strategy
If a node is behind multiple heights, in principle, we could 
- request certificates and blocks for multiple heights in parallel
- employ advanced schemes of incentive-aligned strategies which server to ask for which height

In this version we have encoded a very basic mechanism
- the client uses the information from the received `StatusMsg`s to record who can provide what information
- (A) when a node falls behind
    - it requests a certificate for the next height from one of the servers that is reported that it has this information
    - when the server receives the certificate, 
        - it feeds the certificate into the incoming certificate buffer of the node, and
        - it requests the block from the same server
    - when the server receives the block, 
        - it feeds the block into the incoming block buffer of the node,
        - if there are still heights missing, we repeat from (A)   

In the section on [Issues](#issues) below we will discuss future improvements.

## Formalizing the protocol in Quint

We have formalized the synchronization protocol in Quint.
To do so, we abstracted away many details not relevant to the understanding of the protocol. 
The [specification](../../quint/specs/blocksync/) includes:

- protocol functionality: main complexity in the client, where it maintains statuses,  requests data, and feeds received data into consensus
- state machine: We have put the synchronization on-top-of the consensus specification. For analysis, we may abstract consensus in the future. 
- invariants (that have been preliminarily tested) and temporal formulas (that are just written but have not been investigated further)

### Protocol functionality

This contains mainly the following functions (and their auxiliary functions):

- `pure def bsyncClient (s: BsyncClient) : ClientResult`
    - this encodes what happens during a step of a client:
        1. update peer statuses, 
        2. if there is no open request, request something
        3. otherwise check whether we have a response and act accordingly

- `pure def syncStatus (server: Server) : StatusMsg`
    - look into the block store of the node, generate a status message

- `pure def syncServer (s: Server) : ServerOutput`
    - picks an incoming request (if there is any), and responds the required data
	


### State Machine

The Quint specification works on top of the consensus state machine. We added the following variables

```bluespec
var bsyncClients: Address -> BsyncClient
var bsyncServers: Address -> Server

var statusBuffer : Address -> Set[StatusMsg]
var requestsBuffer : Address -> Set[RequestMsg]
var responsesBuffer : Address -> Set[ResponseMsg]
```

We added the following actions for a correct process `v`:
- `syncDeliverReq(v)`
- `syncDeliverResp(v)`
- `syncDeliverStatus(v)`
- `syncStepClient(v)`
- `syncStatusStep(v)`
- `syncStepServer(v)`
- `syncUpdateServer(v)`

The deliver actions just take a message out of the corresponding network buffer, and puts it into the incoming buffer of node `v`.
The next three actions just execute the matching functions discussed above.
The last action updates the synchronization server state with the latest content of the node's blockchain.

#### syncStepClient
There are two types of effects this action can have.
It can lead to a request message being sent to a server, in which case the message is place in the `requestsBuffer` towards the server.
The second effect is that when the client learns a certificate or a proposal, it will be put into an incoming buffer of a node (from which the consensus logic can later take it out and act on it).

#### syncStatusStep
A status message is broadcast, that is, the message is put into the `statusBuffer` towards all nodes.

#### syncStepServer
I a request is served, the response message is put into the `responsesBuffer` towards the requesting client.


### Invariants and temporal formulas

TODO for the retreat

## Issues

The description refers to a minimal version of the synchronization protocol.
Its roadmap can be found in the issue [#425](https://github.com/informalsystems/malachite/issues/425). 