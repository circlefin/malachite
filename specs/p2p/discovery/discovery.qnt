// -*- mode: Bluespec; -*-

module discovery {
    const PEERS: int
    const BOOTSTRAP_NODES: PeerId -> Set[PeerId]

    type PeerId = int

    val peers: Set[PeerId] = 0.to(PEERS - 1) // The set of all peer IDs

    // Remove a key from a map
    pure def mapRemove(m: PeerId -> Set[PeerId], key: PeerId): PeerId -> Set[PeerId] = {
        m.keys().filter(k => k != key).fold(Map(), (acc, k) => {
            acc.put(k, m.get(k))
        })
    }

    // *************************************************************************
    // Peer state
    // *************************************************************************

    type PeerState = {
        localPeers: Set[PeerId],
        bootstrapNodes: Set[PeerId],
        toContact: Set[PeerId],
        contacted: Set[PeerId],
        pendingRequests: Set[PeerId],
        incomingRequests: PeerId -> Set[PeerId],
        incomingResponses: PeerId -> Set[PeerId],
        /*
         * A peer is idle if it has no actions to perform. Note that a peer
         * can switch multiple times between idle and non-idle states!
         * This state is used to determine when the protocol has finished.
         * Indeed, all peers are idle if and only if the protocol has
         * finished.
         */
        idle: bool
    }

    // Create a new peer state with the given peer ID.
    pure def newPeerState(id: PeerId): PeerState = {
        localPeers: Set(),
        bootstrapNodes: BOOTSTRAP_NODES.get(id),
        toContact: Set((id + 1) % PEERS),
        contacted: Set(),
        pendingRequests: Set(),
        incomingRequests: Map(),
        incomingResponses: Map(),
        idle: false
    }

    // Check if a peer is idle
    pure def isIdle(peerState: PeerState): bool = {
        all {
            peerState.toContact == Set(),
            peerState.pendingRequests == Set(),
            peerState.incomingRequests == Map(),
            peerState.incomingResponses == Map(),
        }
    }

    // *************************************************************************
    // State
    // *************************************************************************

    // The discovery state is a map from peer IDs to their respective peer states
    type State = PeerId -> PeerState

    var state: State

    // Create a new state
    pure def newState(): State = {
        peers.mapBy(id => newPeerState(id))
    }

    // Filter peers based on a predicate
    pure def filterPeers(state: State, p: PeerState => bool): Set[PeerId] = {
        peers.filter(id => p(state.get(id)))
    }

    // Contact a peer
    pure def contactPeer(state: State, peerId: PeerId, contactPeerId: PeerId): State = {
        val peerState = state.get(peerId)

        state.setBy(contactPeerId, old => {
            ...old,
            incomingRequests: old.incomingRequests.put(
                peerId,
                peerState.localPeers.union(peerState.bootstrapNodes)
            ),
        })
    }

    // The contact action handler
    pure def contactHandler(state: State, peerId: PeerId): State = {
        state.get(peerId).toContact.fold(state, (s, p) => {
            contactPeer(s, peerId, p)
        })
            // Adjust the state of the requesting peer
            .setBy(peerId, old => {
                ...old,
                toContact: Set(),
                contacted: old.contacted.union(old.toContact),
                pendingRequests: old.pendingRequests.union(old.toContact),
            })
    }

    // The process request action handler
    pure def processRequestHandler(state: State, peerId: PeerId, requestPeerId: PeerId): State = {
        val peerState = state.get(peerId)

        // Add the requesting peer to the local peers and add the received peers
        // list to the toContact set
        state.setBy(peerId, old => {
            ...old,
            localPeers: old.localPeers.union(Set(requestPeerId)),
            toContact: old.incomingRequests.get(requestPeerId)
                .exclude(old.contacted)
                .exclude(Set(peerId))
                .union(old.toContact),
        })
            // Remove the requesting peer from the incoming requests
            .setBy(peerId, old => {
                ...old,
                incomingRequests: mapRemove(old.incomingRequests, requestPeerId)
            })
            // Check if the peer is now idle
            .setBy(peerId, old => {
                ...old,
                idle: isIdle(old)
            })
            // Add the response to the requesting peer
            .setBy(requestPeerId, old => {
                ...old,
                incomingResponses: old.incomingResponses.put(
                    peerId,
                    // We use the old state to avoid sending to the requesting peer
                    // peers that it already knows
                    peerState.localPeers.union(peerState.bootstrapNodes)
                )
            })
    }

    // The process response action handler
    pure def processResponseHandler(state: State, peerId: PeerId, responsePeerId: PeerId): State = {
        // Add the responding peer to the local peers and add the received peers
        // list to the toContact set
        state.setBy(peerId, old => {
            ...old,
            localPeers: old.localPeers.union(Set(responsePeerId)),
            toContact: old.incomingResponses.get(responsePeerId)
                .exclude(old.contacted)
                .exclude(Set(peerId))
                .union(old.toContact),
        })
            // Remove the response peer from the pending requests and incoming responses
            .setBy(peerId, old => {
                ...old,
                pendingRequests: old.pendingRequests.exclude(Set(responsePeerId)),
                incomingResponses: mapRemove(old.incomingResponses, responsePeerId)
            })
            // Check if the peer is now idle
            .setBy(peerId, old => {
                ...old,
                idle: isIdle(old)
            })
    }

    // *************************************************************************
    // Actions
    // *************************************************************************

    // The init action initializes the state
    action init = {
        state' = newState
    }

    // The step action makes a step in the protocol
    action step = {
        /*
         * A given action is valid to be chosen if there is at least one peer
         * that can perform it. Refer to the peers filtering of each action to
         * see the conditions that a peer must satisfy to be able to perform
         * the action.
         */
        any {
            contact,
            processRequest,
            processResponse,
        }
    }

    /*
     * The contact action sends a request to every peer listed in the toContact
     * set of the chosen peer (also called the requesting peer). Each request is
     * accompanied by the list of peers that the requesting peer knows.
     */
    action contact = {
        val validPeers = filterPeers(state, p => p.toContact != Set())

        all {
            validPeers != Set(),

            nondet peerId = validPeers.oneOf()

            state' = contactHandler(state, peerId)
        }
    }

    /*
     * The processRequest action processes a request from a peer. The request
     * contains the list of peers that the requesting peer knows. The chosen
     * peer (also called the responding peer) will add the requesting peer to
     * its local peers and will add the received peers list to the toContact set.
     * Finally, the responding peer will send a response to the requesting peer
     * containing the list of peers that the responding peer knows.
     */
    action processRequest = {
        val validPeers = filterPeers(state, p => p.incomingRequests != Map())

        all {
            validPeers != Set(),

            nondet peerId = validPeers.oneOf()
            // chooseSome() is not implemented yet, so we use a fold() trick to
            // simulate it
            // val requestPeerId = state.get(peerId).incomingRequests.keys().chooseSome()
            val requestPeerId = state.get(peerId).incomingRequests.keys().fold(0, (acc, p) => p)

            state' = processRequestHandler(state, peerId, requestPeerId)
        }
    }

    /*
     * The processResponse action processes a response from a peer. The response
     * contains the list of peers that the responding peer knows. The chosen
     * peer (also called the requesting peer) will add the responding peer to
     * its local peers and will add the received peers list to the toContact set.
     */
    action processResponse = {
        val validPeers = filterPeers(state, p => p.incomingResponses != Map())

        all {
            validPeers != Set(),

            nondet peerId = validPeers.oneOf()
            // chooseSome() is not implemented yet, so we use a fold() trick to
            // simulate it
            // val responsePeerId = state.get(peerId).incomingResponses.keys().chooseSome()
            val responsePeerId = state.get(peerId).incomingResponses.keys().fold(0, (acc, p) => p)

            state' = processResponseHandler(state, peerId, responsePeerId)
        }
    }

    // *************************************************************************
    // Invariants
    // *************************************************************************

    // Compute the total size of a set of sets
    pure def sumSizeOf(peers: Set[PeerId], f: PeerId => Set[PeerId]): int = {
        peers.fold(0, (acc, p) => acc + f(p).size())
    }

    // The protocol is done if and only if all peers are idle
    val isDone = peers.forall(p => state.get(p).idle)
    val inv = if (isDone) {
        // Check that all peers know all other peers except themselves
        peers.forall(p => state.get(p).localPeers == peers.exclude(Set(p)))
    } else {
        // Check that the number of pending requests is equal to the sum of incoming requests and responses
        val pendingRequests = sumSizeOf(peers, p => state.get(p).pendingRequests)
        val incomingRequests = sumSizeOf(peers, p => state.get(p).incomingRequests.keys())
        val incomingResponses = sumSizeOf(peers, p => state.get(p).incomingResponses.keys())

        pendingRequests == incomingRequests + incomingResponses
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
