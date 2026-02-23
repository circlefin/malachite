# Malachite ValueSync Protocol

The challenge we address with ValueSync is long-term stability:
When running for a long time, the consensus mechanism alone may not be enough to keep the system alive.
If nodes are down or disconnected for some (extended period of) time, they may fall behind several heights.
While the consensus algorithm is fault-tolerant, if too many nodes fall behind, and are not able to catch up, eventually we might not have sufficiently many validators synchronized at the top of the chain to make progress.

## MVP Specification

This specification refers to the initial, Minimum Viable Product (MVP), version of the ValueSync protocol.
Notice that is was originally named "Blocksync", for instance in its [tracking issue](https://github.com/circlefin/malachite/issues/425).

We consider a composition of:

- **consensus**. The consensus node: Executing consensus iteratively over multiple heights, and storing the decided values;
- **client**. ValueSync Client: That tries to obtain data (certificates, values) in order to decide quickly, in case the node has fallen behind and other nodes have already decided on values;
- **server**. ValueSync Server. Provides data about decided values to clients.

### Outline of the protocol

The rough idea of the protocol is the following:
- Consensus, client and server run in parallel on a node
- The client observes the height of its local consensus instance
- The server regularly announces the height of its local consensus instance to the network
- When a client observes that the local height is smaller than a remote height, it requests a missing height:
  the commit (a certificate consisting of 2f+1 `Precommit` messages) and the committed value (`Proposal`)
- When a server receives such a request, it obtains the required information from the local value store, and sends it to the client
- When a client receives a response (certificate or proposal or both), it delivers this information to the consensus logic
- The consensus logic (driver) handles this incoming information in the same way as it would handle it if it came from "regular" consensus operation.

### Design decision

Observe that in the protocol description above, we have already taken a big design decision, namely, that the components consensus, client, and server run in parallel. This has been a conscious decision.

There are other potential designs, where the protocols alternate: when a node figures out that it has fallen behind several heights, it switches from consensus mode to synchronization mode, and then, when it has finished synchronizing, it switches back to consensus.
This approach has two downsides: (1) consensus values are decided in (at least) two locations of the code, consensus and synchronization, and (2) what are the precise conditions to switch between the two modes, and what are the expectations about the local state (e.g., incoming message buffers, etc.) when such a switch happens?
Particularly Point 2 is quite hard to get right in a distributed setting.
For instance, a node might locally believe that it is synchronized, while others have actually moved ahead instead.
This creates a lot of algorithmic complexity as well as complications in the analysis of the protocols.

We have thus decided to go for another approach:

- Consensus, client and server run in parallel on a node (we don't need to define switching conditions between Consensus and ValueSync as they are always running together)
- The consensus logic is the single point where decisions are made (i.e., values are committed)
- ValueSync is just an alternative source for certificates and proposals
- ValueSync can be run as add-on, and doesn't need any change to the consensus mechanism/architecture already implemented/specified in Malachite.
- Coupling of ValueSync client and server to the consensus logic:
    - the server needs read access to the value store in order to retrieve the current height, as well as certificates and values for committed heights
    - the client needs write access to incoming buffers (for certificates and values) of the consensus logic


## Central aspects of ValueSync

### Synchronization Messages

#### Status message

In regular intervals, each server sends out a status message, informing the others of its address and telling it from which height (base) to which height (top) it can provide certificates and values:
```bluespec
type StatusMsg = {
    peer: Address,
    base: Height,
    top: Height
}
```

#### Request message

A client asks a specific peer either for a certificate or for a value at a given height:

```bluespec
type ReqType =
    | SyncCertificate
    | SyncValue
    | SyncValueStoreEntry

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
    | RespValue(Proposal)
    | RespCertificate(Set[Vote])
    | RespValueStoreEntry(ValueStoreEntry)

type ResponseMsg = {
    client: Address,
    server: Address,
    height: Height,
    response: Response,
}
```

### Synchronization Strategy

If a node is behind multiple heights, in principle, we could
- request certificates and values for multiple heights in parallel
- employ advanced schemes of incentive-aligned strategies which server to ask for which height

In this version we have encoded a very basic mechanism
- the client uses the information from the received messages `StatusMsg` to record who can provide what information
- (A) when a node falls behind
    - it requests a certificate for the next height from one of the servers that is reported that it has this information
    - when the client receives the certificate,
        - it feeds the certificate into the incoming certificate buffer of driver, and
        - it requests the value from the same server
    - when the client receives the value,
        - it feeds the value into the incoming value buffer of the node,
        - if there are still heights missing, we repeat from (A)
    - note that the two requests/responses for commits and values can be bundled together

In the section on [Issues](#issues) below we will discuss future improvements.

## Formalizing the protocol in Quint

We have formalized ValueSync in Quint.
To do so, we abstracted away many details not relevant to the understanding of the protocol.
The [specification](./quint/README.md) includes:

- Protocol functionality: main complexity in the client, where it maintains statuses,  requests data, and feeds received data into consensus
- State machine: We have encoded two alternatives
    - We have put the ValueSync on-top-of the consensus specification (`vsyncWithConsensus`). This allows us to simulate consensus and ValueSync in one model.
    - We have encoded a state machine that abstracts away consensus (`vsyncWithMockedConsensus`) that allows us to analyze ValueSync in isolation.
- Invariants (that have been preliminarily tested) and temporal formulas (that are just written but have not been investigated further)

### Protocol functionality

This contains mainly the following functions (and their auxiliary functions):

- `pure def vsyncClientLogic (s: BsyncClient, fullEntries: bool) : ClientResult`
    - this encodes what happens during a step of a client:
        1. update peer statuses,
        2. if there is no open request, request something
        3. otherwise check whether we have a response and act accordingly
        4. `fullEntries` is used to signal whether complete value store entries should be requested (rather than certificate and value separately)

- `pure def vsyncClientTimeout (s: BsyncClient) : ClientResult`
    - this encodes what happens when a request timeouts:
        1. update peer statuses,
        2. send the request to another suitable peer

- `pure def syncStatus (server: Server) : StatusMsg`
    - look into the value store of the node, generate a status message

- `pure def syncServer (s: Server) : ServerOutput`
    - picks an incoming request (if there is any), and responds the required data
	


### State Machine

The Quint specification works on top of the consensus state machine. We added the following variables

```bluespec
var vsyncClients: Address -> BsyncClient
var vsyncServers: Address -> Server

var statusBuffer : Address -> Set[StatusMsg]
var requestsBuffer : Address -> Set[RequestMsg]
var responsesBuffer : Address -> Set[ResponseMsg]
```

We have encoded two different state machines (1) that interacts with the consensus specification and (2) that abstracts consensus. In order to do so, we have separated actions into several modules.
The specification `syncStatemachine.qnt` encodes all actions that are touching only ValueSync state, that is, the don't interact with consensus.
These are the actions for a correct process `v`:
- `syncDeliverReq(v)`
- `syncDeliverResp(v)`
- `syncDeliverStatus(v)`
- `syncStatusStep(v)`
- `syncStepServer(v)`
- `syncClientTimeout(v)`

The deliver actions just take a message out of the corresponding network buffer, and puts it into the incoming buffer of node `v`. The other three actions just call the corresponding functions.

The following to actions interact with consensus and thus there are two versions for the two mentioned state machines.
- `syncStepClient`: executes the client function and potentially feeds certificates or values into consensus
- `syncUpdateServer`: updates the synchronization server state with the latest content of the node's valuechain.




#### syncStepClient

There are two types of effects this action can have.
It can lead to a request message being sent to a server, in which case the message is place in the `requestsBuffer` towards the server.
The second effect is that when the client learns a certificate or a proposal, it will be put into an incoming buffer of a node (from which the consensus logic can later take it out and act on it).

#### syncStatusStep

A status message is broadcast, that is, the message is put into the `statusBuffer` towards all nodes.

#### syncStepServer

I a request is served, the response message is put into the `responsesBuffer` towards the requesting client.


### Invariants and temporal formulas

For details we refer to the [state machine in Quint](./quint/vsyncStatemachine.qnt), and the [analysis documentation](./quint/README.md).

## Issues

The description refers to a minimal version of the synchronization protocol.
Its roadmap can be found in the issue [#425](https://github.com/circlefin/malachite/issues/425).
