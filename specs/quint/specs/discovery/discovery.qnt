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
            processOneRequest,
            processOneResponse,
        }
    }

    def filterPeers(p: State => bool): Set[PeerId] = {
        peers.filter(id => p(peerStates.get(id)))
    }

    def mapRemove(m: PeerId -> Set[PeerId], key: PeerId): PeerId -> Set[PeerId] = {
        m.keys().filter(k => k != key).fold(Map(), (acc, k) => {
            acc.put(k, m.get(k))
        })
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

    action processOneRequest = {
        val validPeers = filterPeers(p => p.incomingRequests != Map())

        all {
            validPeers != Set(),

            nondet peerId = validPeers.oneOf()
            nondet requestPeerId = peerStates.get(peerId).incomingRequests.keys().oneOf()

            val state = peerStates.get(peerId)

            val newLocalPeers = state.localPeers.union(Set(requestPeerId))
            val newToContact = state.incomingRequests.get(requestPeerId)
                .exclude(state.contacted)
                .exclude(Set(peerId))
                .union(state.toContact)
            val newIncomingRequests = mapRemove(state.incomingRequests, requestPeerId)
            val idle = all {
                newToContact == Set(),
                state.pendingRequests == Set(),
                state.incomingResponses == Map(),
            }

            val newState = peerStates.set(peerId, {
                ...state,
                localPeers: newLocalPeers,
                toContact: newToContact,
                incomingRequests: newIncomingRequests,
                idle: idle,
            })

            val ps = newState.get(requestPeerId)

            peerStates' = newState.set(requestPeerId, {
                ...ps,
                incomingResponses: ps.incomingResponses.put(
                    peerId,
                    state.localPeers.union(state.bootstrapNodes)
                ),
            })
        }
    }

    action processOneResponse = {
        val validPeers = filterPeers(p => p.incomingResponses != Map())

        all {
            validPeers != Set(),

            nondet peerId = validPeers.oneOf()
            nondet responsePeerId = peerStates.get(peerId).incomingResponses.keys().oneOf()

            val state = peerStates.get(peerId)

            val newLocalPeers = state.localPeers.union(Set(responsePeerId))
            val newToContact = state.incomingResponses.get(responsePeerId)
                .exclude(state.contacted)
                .exclude(Set(peerId))
                .union(state.toContact)
            val newPendingRequests = state.pendingRequests.exclude(Set(responsePeerId))
            val newIncomingResponses = mapRemove(state.incomingResponses, responsePeerId)
            val idle = all {
                newToContact == Set(),
                newPendingRequests == Set(),
                state.incomingRequests == Map(),
            }

            peerStates' = peerStates.set(peerId, {
                ...state,
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
