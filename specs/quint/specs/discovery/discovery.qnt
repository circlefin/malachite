// -*- mode: Bluespec; -*-

module discovery {
    const PEERS: int
    const BOOTSTRAP_NODES: PeerId -> Set[PeerId]

    type PeerId = int
    type State = {
        localPeers: Set[PeerId],
        bootstrapNodes: Set[PeerId],
        toContact: Set[PeerId],
        contacted: Set[PeerId],
        pendingRequests: Set[PeerId],
        incomingRequests: Set[PeerId],
        incomingResponses: PeerId -> Set[PeerId],
        done: bool
    }

    val peers: Set[PeerId] = 0.to(PEERS - 1)
    var peerStates: PeerId -> State

    action init = {
        peerStates' = peers.mapBy(id => {
            localPeers: Set(),
            bootstrapNodes: BOOTSTRAP_NODES.get(id),
            toContact: Set((id + 1) % PEERS),
            contacted: Set(),
            pendingRequests: Set(),
            incomingRequests: Set(),
            incomingResponses: Map(),
            done: false
        })
    }

    action step = {
        any {
            contact,
            processRequests,
            processResponses,
        }
    }

    def filterPeers(p: State => bool): Set[PeerId] = {
        peers.filter(id => p(peerStates.get(id)))
    }

    action contact = {
        val validPeers = filterPeers(p => p.toContact != Set())

        all {
            validPeers != Set(),

            nondet peerId = validPeers.oneOf()
            val state = peerStates.get(peerId)

            val newState = state.toContact.fold(peerStates, (s, p) => {
                val ps = s.get(p)
                s.set(p, {
                    ...ps,
                    incomingRequests: ps.incomingRequests.union(Set(peerId))
                })
            })

            peerStates' = newState.set(peerId, {
                ...state,
                toContact: Set(),
                contacted: state.contacted.union(state.toContact),
                pendingRequests: state.pendingRequests.union(state.toContact),
            })
        }
    }

    action processRequests = {
        val validPeers = filterPeers(p => p.incomingRequests != Set())

        all {
            validPeers != Set(),

            nondet peerId = validPeers.oneOf()
            val state = peerStates.get(peerId)

            val newState = state.incomingRequests.fold(peerStates, (s, p) => {
                val ps = s.get(p)
                s.set(p, {
                    ...ps,
                    incomingResponses: ps.incomingResponses.put(peerId, state.localPeers.union(state.bootstrapNodes)),
                })
            })

            peerStates' = newState.set(peerId, {
                ...state,
                localPeers: state.localPeers.union(state.incomingRequests),
                incomingRequests: Set(),
            })
        }
    }

    action processResponses = {
        val validPeers = filterPeers(p => p.incomingResponses != Map())

        all {
            validPeers != Set(),

            nondet peerId = validPeers.oneOf()
            val state = peerStates.get(peerId)

            val newState = state.incomingResponses.keys().fold(peerStates, (s, p) => {
                val localState = s.get(peerId)
                val newPeersToContact = state.incomingResponses.get(p)
                    .exclude(state.contacted)
                    .exclude(Set(peerId))
                s.set(peerId, {
                    ...localState,
                    localPeers: localState.localPeers.union(Set(p)),
                    toContact: localState.toContact.union(newPeersToContact),
                    pendingRequests: localState.pendingRequests.exclude(Set(p)),
                })
            })

            val newPeerState = newState.get(peerId)

            val done = all {
                newPeerState.localPeers != Set(),
                newPeerState.toContact == Set(),
                newPeerState.pendingRequests == Set(),
            }

            peerStates' = newState.set(peerId, {
                ...newState.get(peerId),
                incomingResponses: Map(),
                done: done,
            })
        }
    }

    def sumSizeOf(peers: Set[PeerId], f: PeerId => Set[PeerId]): int = {
        peers.fold(0, (acc, p) => acc + f(p).size())
    }

    val isDone = peers.forall(p => peerStates.get(p).done)
    val inv = if (isDone) {
        peers.forall(p => peerStates.get(p).localPeers == peers.exclude(Set(p)))
    } else {
        val pendingRequests = sumSizeOf(peers, p => peerStates.get(p).pendingRequests)
        val incomingRequests = sumSizeOf(peers, p => peerStates.get(p).incomingRequests)
        val incomingResponses = sumSizeOf(peers, p => peerStates.get(p).incomingResponses.keys())

        pendingRequests == incomingRequests + incomingResponses
    }
}

module disc5_not_strongly_connected {
    import discovery(PEERS=5, BOOTSTRAP_NODES=Map(0 -> Set(1), 1 -> Set(0), 2 -> Set(1, 3), 3 -> Set(4), 4 -> Set(3))).*

    run run1 = init.then(100.reps(_ => step))
}
