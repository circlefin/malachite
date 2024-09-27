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
        incomingRequests: PeerId -> Set[PeerId],
        incomingResponses: PeerId -> Set[PeerId],
        idle: bool
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
            incomingRequests: Map(),
            incomingResponses: Map(),
            idle: false
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
                    incomingRequests: ps.incomingRequests.put(
                        peerId,
                        state.localPeers.union(state.bootstrapNodes)
                    ),
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
        val validPeers = filterPeers(p => p.incomingRequests != Map())

        all {
            validPeers != Set(),

            nondet peerId = validPeers.oneOf()

            val newState = peerStates.get(peerId).incomingRequests.keys().fold(peerStates, (s, p) => {
                val localState = s.get(peerId)
                val newPeersToContact = localState.incomingRequests.get(p)
                    .exclude(localState.contacted)
                    .exclude(Set(peerId))
                val newState = s.set(peerId, {
                    ...localState,
                    localPeers: localState.localPeers.union(Set(p)),
                    toContact: localState.toContact.union(newPeersToContact),
                })

                val ps = newState.get(p)
                newState.set(p, {
                    ...ps,
                    incomingResponses: ps.incomingResponses.put(
                        peerId,
                        localState.localPeers.union(localState.bootstrapNodes)
                    ),
                })
            })

            val newPeerState = newState.get(peerId)

            val idle = all {
                newPeerState.toContact == Set(),
                newPeerState.pendingRequests == Set(),
                newPeerState.incomingResponses == Map(),
            }

            peerStates' = newState.set(peerId, {
                ...newPeerState,
                incomingRequests: Map(),
                idle: idle,
            })
        }
    }

    action processResponses = {
        val validPeers = filterPeers(p => p.incomingResponses != Map())

        all {
            validPeers != Set(),

            nondet peerId = validPeers.oneOf()

            val newState = peerStates.get(peerId).incomingResponses.keys().fold(peerStates, (s, p) => {
                val localState = s.get(peerId)
                val newPeersToContact = localState.incomingResponses.get(p)
                    .exclude(localState.contacted)
                    .exclude(Set(peerId))
                s.set(peerId, {
                    ...localState,
                    localPeers: localState.localPeers.union(Set(p)),
                    toContact: localState.toContact.union(newPeersToContact),
                    pendingRequests: localState.pendingRequests.exclude(Set(p)),
                })
            })

            val newPeerState = newState.get(peerId)

            val idle = all {
                newPeerState.toContact == Set(),
                newPeerState.pendingRequests == Set(),
                newPeerState.incomingRequests == Map(),
            }

            peerStates' = newState.set(peerId, {
                ...newPeerState,
                incomingResponses: Map(),
                idle: idle,
            })
        }
    }

    def sumSizeOf(peers: Set[PeerId], f: PeerId => Set[PeerId]): int = {
        peers.fold(0, (acc, p) => acc + f(p).size())
    }

    val isDone = peers.forall(p => peerStates.get(p).idle)
    val inv = if (isDone) {
        peers.forall(p => peerStates.get(p).localPeers == peers.exclude(Set(p)))
    } else {
        val pendingRequests = sumSizeOf(peers, p => peerStates.get(p).pendingRequests)
        val incomingRequests = sumSizeOf(peers, p => peerStates.get(p).incomingRequests.keys())
        val incomingResponses = sumSizeOf(peers, p => peerStates.get(p).incomingResponses.keys())

        pendingRequests == incomingRequests + incomingResponses
    }
}

module disc5_not_strongly_connected {
    import discovery(PEERS=5, BOOTSTRAP_NODES=Map(0 -> Set(1), 1 -> Set(0), 2 -> Set(1, 3), 3 -> Set(4), 4 -> Set(3))).*

    run run1 = init.then(100.reps(_ => step))
}
