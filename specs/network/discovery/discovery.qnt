// -*- mode: Bluespec; -*-

module discovery {
    const PEERS: int
    const BOOTSTRAP_NODES: PeerId -> Set[PeerId]

    type PeerId = int

    val peers: Set[PeerId] = 0.to(PEERS - 1) // The set of all peer IDs

    // *************************************************************************
    // Message
    // *************************************************************************

    type MessageType = Request | Response

    type Message =  {
        msgType: MessageType,
        payload: Set[PeerId],
        sender: PeerId,
        receiver: PeerId
    }

    // *************************************************************************
    // Peer state
    // *************************************************************************

    type PeerState = {
        id: PeerId,
        localPeers: Set[PeerId],
        bootstrapNodes: Set[PeerId],
        toContact: Set[PeerId],
        contacted: Set[PeerId],
        pendingRequests: Set[PeerId],
        incomingRequests: Set[Message],
        incomingResponses: Set[Message],
        /*
         * A peer is idle if it has no actions to perform. Note that a peer
         * can switch multiple times between idle and non-idle states!
         * This state is used to determine when the protocol has finished.
         * Indeed, the protocol has finished when all peers are idle and there
         * are no more messages to deliver.
         */
        idle: bool
    }

    // Create a new peer state with the given peer ID.
    pure def newPeerState(id: PeerId): PeerState = {
        id: id,
        localPeers: Set(),
        bootstrapNodes: BOOTSTRAP_NODES.get(id),
        toContact: Set((id + 1) % PEERS),
        contacted: Set(),
        pendingRequests: Set(),
        incomingRequests: Set(),
        incomingResponses: Set(),
        idle: false
    }

    // Check if a peer can perform a step
    pure def canPerformStep(peerState: PeerState): bool = {
        any {
            peerState.toContact != Set(),
            peerState.incomingRequests != Set(),
            peerState.incomingResponses != Set()
        }
    }

    // Check if a peer is idle
    pure def isIdle(peerState: PeerState): bool = {
        all {
            peerState.toContact == Set(),
            peerState.pendingRequests == Set(),
            peerState.incomingRequests == Set(),
            peerState.incomingResponses == Set()
        }
    }

    // Handle the contact action for a peer
    pure def contactHandler(peerState: PeerState): {newPeerState: PeerState, messages: Set[Message]} = {
        {
            newPeerState: {
                ...peerState,
                toContact: Set(),
                contacted: peerState.contacted.union(peerState.toContact),
                pendingRequests: peerState.pendingRequests.union(peerState.toContact)
            },
            messages: peerState.toContact.map(p => {
                msgType: Request,
                payload: peerState.localPeers.union(peerState.bootstrapNodes),
                sender: peerState.id,
                receiver: p
            })
        }
    }

    // Handle the processRequest action for a peer
    pure def processRequestHandler(peerState: PeerState): {newPeerState: PeerState, messages: Set[Message]} = {
        // The fold simulates the function chooseSome(), which is not yet supported
        val message = peerState.incomingRequests.fold({msgType: Request, payload: Set(), sender: 0, receiver: 0}, (acc, m) => m)

        // The use of an intermediate state is necessary to correctly update the
        // idle state of the peer
        val interPeerState = {
            ...peerState,
            localPeers: peerState.localPeers.union(Set(message.sender)),
            toContact: peerState.toContact
                .union(message.payload)
                .exclude(Set(peerState.id))
                .exclude(peerState.contacted),
            incomingRequests: peerState.incomingRequests.exclude(Set(message))
        }

        {
            newPeerState: {
                ...interPeerState,
                idle: isIdle(interPeerState)
            },
            messages: Set({
                msgType: Response,
                payload: peerState.localPeers.union(peerState.bootstrapNodes),
                sender: peerState.id,
                receiver: message.sender
            })
        }
    }

    // Handle the processResponse action for a peer
    pure def processResponseHandler(peerState: PeerState): {newPeerState: PeerState, messages: Set[Message]} = {
        // The fold simulates the function chooseSome(), which is not yet supported
        val message = peerState.incomingResponses.fold({msgType: Request, payload: Set(), sender: 0, receiver: 0}, (acc, m) => m)

        // The use of an intermediate state is necessary to correctly update the
        // idle state of the peer
        val interPeerState = {
            ...peerState,
            localPeers: peerState.localPeers.union(Set(message.sender)),
            toContact: peerState.toContact
                .union(message.payload)
                .exclude(Set(peerState.id))
                .exclude(peerState.contacted),
            pendingRequests: peerState.pendingRequests.exclude(Set(message.sender)),
            incomingResponses: peerState.incomingResponses.exclude(Set(message))
        }

        {
            newPeerState: {
                ...interPeerState,
                idle: isIdle(interPeerState)
            },
            messages: Set()
        }
    }

    // *************************************************************************
    // State
    // *************************************************************************

    // The discovery state is made of the state of each peer and the messages buffer
    type State = {
        peersStates: PeerId -> PeerState,
        messagesBuffer: Set[Message]
    }

    var state: State

    // Create a new state
    pure def newState(): State = {
        peersStates: peers.mapBy(id => newPeerState(id)),
        messagesBuffer: Set()
    }

    // Filter peers based on a predicate
    pure def filterPeers(state: State, p: PeerState => bool): Set[PeerId] = {
        peers.filter(id => p(state.peersStates.get(id)))
    }

    // *************************************************************************
    // Actions
    // *************************************************************************

    // The init action initializes the state
    action init = {
        state' = newState
    }

    action step = {
        val validPeers = filterPeers(state, p => canPerformStep(p))

        any {
            // Perform a step on a peer (if any)
            all {
                validPeers != Set(),

                nondet peerId = validPeers.oneOf()
                peerStep(peerId)
            },

            // OR

            // Deliver a message (if any)
            all {
                state.messagesBuffer != Set(),

                nondet message = state.messagesBuffer.oneOf()
                deliver(message)
            }
        }
    }

    // The peerStep action makes a peer perform an available action
    action peerStep(peerId) = {
        any {
            all {
                state.peersStates.get(peerId).toContact != Set(),
                contact(peerId)
            },
            all {
                state.peersStates.get(peerId).incomingRequests != Set(),
                processRequest(peerId)
            },
            all {
                state.peersStates.get(peerId).incomingResponses != Set(),
                processResponse(peerId)
            }
        }
    }

    // The deliver action delivers a message to the corresponding peer
    action deliver(message) = {
        all {
            if (message.msgType == Request) {
                val newPeersStates = state.peersStates.setBy(message.receiver, old => {
                    ...old,
                    incomingRequests: old.incomingRequests.union(Set(message))
                })

                state' = {
                    peersStates: newPeersStates,
                    messagesBuffer: state.messagesBuffer.exclude(Set(message))
                }
            } else {
                val newPeersStates = state.peersStates.setBy(message.receiver, old => {
                    ...old,
                    incomingResponses: old.incomingResponses.union(Set(message))
                })

                state' = {
                    peersStates: newPeersStates,
                    messagesBuffer: state.messagesBuffer.exclude(Set(message))
                }
            }
        }
    }

    // The contact action makes a peer contact its toContact set
    action contact(peerId) = {
        val result = contactHandler(state.peersStates.get(peerId))

        state' = {
            peersStates: state.peersStates.set(peerId, result.newPeerState),
            messagesBuffer: state.messagesBuffer.union(result.messages)
        }
    }

    // The processRequest action processes an incoming request
    action processRequest(peerId) = {
        val result = processRequestHandler(state.peersStates.get(peerId))

        state' = {
            peersStates: state.peersStates.set(peerId, result.newPeerState),
            messagesBuffer: state.messagesBuffer.union(result.messages)
        }
    }

    // The processResponse action processes an incoming response
    action processResponse(peerId) = {
        val result = processResponseHandler(state.peersStates.get(peerId))

        state' = {
            peersStates: state.peersStates.set(peerId, result.newPeerState),
            messagesBuffer: state.messagesBuffer.union(result.messages)
        }
    }

    // *************************************************************************
    // Invariants
    // *************************************************************************

    // Compute the total size of a set of sets
    pure def sumSizeOf(peers: Set[PeerId], f: PeerId => int): int = {
        peers.fold(0, (acc, p) => acc + f(p))
    }

    // The protocol is done if and only if all peers are idle
    val isDone = all {
        peers.forall(p => state.peersStates.get(p).idle),
        state.messagesBuffer == Set()
    }
    val inv = if (isDone) {
        // Check that all peers know all other peers except themselves
        peers.forall(p => state.peersStates.get(p).localPeers == peers.exclude(Set(p)))
    } else {
        // Check that the number of pending requests is equal to the sum of
        // incoming requests, responses, and the number of messages in the buffer
        val pendingRequests = sumSizeOf(peers, p => state.peersStates.get(p).pendingRequests.size())
        val incomingRequests = sumSizeOf(peers, p => state.peersStates.get(p).incomingRequests.size())
        val incomingResponses = sumSizeOf(peers, p => state.peersStates.get(p).incomingResponses.size())
        val messagesBuffer = state.messagesBuffer.size()

        pendingRequests == incomingRequests + incomingResponses + messagesBuffer
    }
}

module disc5_not_strongly_connected {
    /**
     * Testing the following not strongly connected bootstrap sets graph:
     *     0 <--> 1 <--- 2 ---> 3 <--> 4
     */
    import discovery(PEERS=5, BOOTSTRAP_NODES=Map(0 -> Set(1), 1 -> Set(0), 2 -> Set(1, 3), 3 -> Set(4), 4 -> Set(3))).*

    run run1 = init.then(100.reps(_ => step))
}
