module discovery {
    const PEERS: int

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
            bootstrapNodes: Set((id + 1) % PEERS),
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
            noPendingRequest
        }
    }

    action contact = {
        val validPeers = peerStates.keys().filter(p => {
            peerStates.get(p).toContact != Set()
        })

        all {
            validPeers != Set(),

            nondet peerId = validPeers.oneOf()
            val state = peerStates.get(peerId)

            val newState = state.toContact.fold(peerStates, (s, p) => {
                val ps = s.get(p)
                s.set(p, {...ps, incomingRequests: ps.incomingRequests.union(Set(peerId))})
            })

            peerStates' = newState.set(peerId, {
                ...state,
                toContact: Set(),
                contacted: state.contacted.union(state.toContact),
                pendingRequests: state.toContact,
            })
        }
    }

    action processRequests = {
        val validPeers = peerStates.keys().filter(p => {
            peerStates.get(p).incomingRequests != Set()
        })

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
        val validPeers = peerStates.keys().filter(p => {
            peerStates.get(p).incomingResponses != Map()
        })

        all {
            validPeers != Set(),

            nondet peerId = validPeers.oneOf()
            val state = peerStates.get(peerId)

            val newState = state.incomingResponses.keys().fold(peerStates, (s, p) => {
                val localState = s.get(peerId)
                s.set(peerId, {
                    ...localState,
                    localPeers: localState.localPeers.union(Set(p)),
                    toContact: localState.toContact.union(state.incomingResponses.get(p).exclude(state.contacted).exclude(Set(peerId))),
                    pendingRequests: localState.pendingRequests.exclude(Set(p)),
                })
            })

            peerStates' = newState.set(peerId, {
                ...newState.get(peerId),
                incomingResponses: Map(),
            })
        }
    }

    action noPendingRequest = {
        val validPeers = peerStates.keys().filter(p => {
            val peerState = peerStates.get(p)

            peerState.localPeers != Set()
                and peerState.toContact == Set()
                and peerState.pendingRequests == Set()
                and peerState.incomingResponses == Map()
                and peerState.done == false
        })

        all {
            validPeers != Set(),

            nondet peerId = validPeers.oneOf()
            val state = peerStates.get(peerId)
            
            peerStates' = peerStates.set(peerId, {...state, done: true})
        }
    }

    val isDone = peers.forall(p => peerStates.get(p).done)
    val inv = if (isDone) peers.forall(p => peerStates.get(p).localPeers == peers.exclude(Set(p))) else true
}

module disc5 {
    import discovery(PEERS=5).*

    run run1 = init.then(100.reps(_ => step))
}
