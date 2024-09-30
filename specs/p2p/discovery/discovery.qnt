// -*- mode: Bluespec; -*-

module discovery {
    const PEERS: int
    const BOOTSTRAP_NODES: PeerId -> Set[PeerId]

    type PeerId = int

    val peers: Set[PeerId] = 0.to(PEERS - 1) // The set of all peer IDs

    // *************************************************************************
    // State
    // *************************************************************************

    type PeerState = {
        localPeers: Set[PeerId],
        bootstrapNodes: Set[PeerId],
        toContact: Set[PeerId],
        contacted: Set[PeerId],
        pendingRequests: Set[PeerId],
        incomingRequests: PeerId -> Set[PeerId],
        incomingResponses: PeerId -> Set[PeerId],
        idle: bool
    }

    // The discovery state is a map from peer IDs to their respective peer states
    type State = PeerId -> PeerState

    var state: State

    // Filter peers based on a predicate
    pure def filterPeers(state: State, p: PeerState => bool): Set[PeerId] = {
        peers.filter(id => p(state.get(id)))
    }

    // Remove a key from a map
    pure def mapRemove(m: PeerId -> Set[PeerId], key: PeerId): PeerId -> Set[PeerId] = {
        m.keys().filter(k => k != key).fold(Map(), (acc, k) => {
            acc.put(k, m.get(k))
        })
    }

    pure def contactPeer(state: State, peerId: PeerId, peer: PeerId): State = {
        val peerState = state.get(peerId)
        val ps = state.get(peer)

        state.set(peer, {
            ...ps,
            incomingRequests: ps.incomingRequests.put(
                peerId,
                peerState.localPeers.union(peerState.bootstrapNodes)
            ),
        })
    }

    // *************************************************************************
    // Actions
    // *************************************************************************

    action init = {
        state' = peers.mapBy(id => {
            localPeers: Set(),
            bootstrapNodes: BOOTSTRAP_NODES.get(id),
            toContact: Set((id + 1) % PEERS),
            contacted: Set(),
            pendingRequests: Set(),
            incomingRequests: Map(),
            incomingResponses: Map(),
            /*
             * A peer is idle if it has no actions to perform. Note that a peer
             * can switch multiple times between idle and non-idle states!
             * This state is used to determine when the protocol has finished.
             * Indeed, all peers are idle if and only if the protocol has
             * finished.
             */
            idle: false
        })
    }

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
     * set of the chosen peer (also called the requesting peer).
     */
    action contact = {
        val validPeers = filterPeers(state, p => p.toContact != Set())

        all {
            validPeers != Set(),

            nondet peerId = validPeers.oneOf()
            val peerState = state.get(peerId)

            val newState = peerState.toContact.fold(state, (s, p) => {
                contactPeer(s, peerId, p)
            })

            // Adjust the state of the requesting peer
            state' = newState.set(peerId, {
                ...peerState,
                toContact: Set(),
                contacted: peerState.contacted.union(peerState.toContact),
                pendingRequests: peerState.pendingRequests.union(peerState.toContact),
            })
        }
    }

    action processRequest = {
        val validPeers = filterPeers(state, p => p.incomingRequests != Map())

        all {
            validPeers != Set(),

            nondet peerId = validPeers.oneOf()
            nondet requestPeerId = state.get(peerId).incomingRequests.keys().oneOf()

            val peerState = state.get(peerId)

            val newLocalPeers = peerState.localPeers.union(Set(requestPeerId))
            val newToContact = peerState.incomingRequests.get(requestPeerId)
                .exclude(peerState.contacted)
                .exclude(Set(peerId))
                .union(peerState.toContact)
            val newIncomingRequests = mapRemove(peerState.incomingRequests, requestPeerId)
            val idle = all {
                newToContact == Set(),
                peerState.pendingRequests == Set(),
                newIncomingRequests == Map(),
                peerState.incomingResponses == Map(),
            }

            val newState = state.set(peerId, {
                ...peerState,
                localPeers: newLocalPeers,
                toContact: newToContact,
                incomingRequests: newIncomingRequests,
                idle: idle,
            })

            val ps = newState.get(requestPeerId)

            state' = newState.set(requestPeerId, {
                ...ps,
                incomingResponses: ps.incomingResponses.put(
                    peerId,
                    peerState.localPeers.union(peerState.bootstrapNodes)
                ),
            })
        }
    }

    action processResponse = {
        val validPeers = filterPeers(state, p => p.incomingResponses != Map())

        all {
            validPeers != Set(),

            nondet peerId = validPeers.oneOf()
            nondet responsePeerId = state.get(peerId).incomingResponses.keys().oneOf()

            val peerState = state.get(peerId)

            val newLocalPeers = peerState.localPeers.union(Set(responsePeerId))
            val newToContact = peerState.incomingResponses.get(responsePeerId)
                .exclude(peerState.contacted)
                .exclude(Set(peerId))
                .union(peerState.toContact)
            val newPendingRequests = peerState.pendingRequests.exclude(Set(responsePeerId))
            val newIncomingResponses = mapRemove(peerState.incomingResponses, responsePeerId)
            val idle = all {
                newToContact == Set(),
                newPendingRequests == Set(),
                peerState.incomingRequests == Map(),
                newIncomingResponses == Map(),
            }

            state' = state.set(peerId, {
                ...peerState,
                localPeers: newLocalPeers,
                toContact: newToContact,
                pendingRequests: newPendingRequests,
                incomingResponses: newIncomingResponses,
                idle: idle,
            })
        }
    }

    def sumSizeOf(peers: Set[PeerId], f: PeerId => Set[PeerId]): int = {
        peers.fold(0, (acc, p) => acc + f(p).size())
    }

    val isDone = peers.forall(p => state.get(p).idle)
    val inv = if (isDone) {
        peers.forall(p => state.get(p).localPeers == peers.exclude(Set(p)))
    } else {
        val pendingRequests = sumSizeOf(peers, p => state.get(p).pendingRequests)
        val incomingRequests = sumSizeOf(peers, p => state.get(p).incomingRequests.keys())
        val incomingResponses = sumSizeOf(peers, p => state.get(p).incomingResponses.keys())

        pendingRequests == incomingRequests + incomingResponses
    }
}

module disc5_not_strongly_connected {
    import discovery(PEERS=5, BOOTSTRAP_NODES=Map(0 -> Set(1), 1 -> Set(0), 2 -> Set(1, 3), 3 -> Set(4), 4 -> Set(3))).*

    run run1 = init.then(100.reps(_ => step))
}
