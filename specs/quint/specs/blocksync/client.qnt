// -*- mode: Bluespec; -*-
// ****************************************************************************
// Synchronizer (client side)
// ****************************************************************************

module client {
	import blocksync.* from "./blocksync"

	/// The state of the synchronizer
	type Synchronizer = {
	    id: Address,
	    height: Height,
	    peers: Set[Address],    // MVP: this is going to be the validator set for now
	                            // so we use addresses. We might use peerID in the future
	    peerStatus: Address -> SyncStatus,
	    openRequests: Set[RequestMsg],
	    lastSyncedHeight: Height,   // "done" if greater than or equal to height
	                                // TODO: we could add buffers for certificates and values
	    // inbuffers
	    statusMsgs: Set[SyncStatusMsg],
	    responses: Set[ResponseMsg],
	}

	type SynchronizerOutput =
	    | SOCertificate(Set[Vote])
	    | SOBlock(Proposal)
	    | SONoOutput

	//
	// Synchronizer functions
	//

	/// Initialize the synchronizer
	pure def initSynchronizer(id: Address, peers: Set[Address]) : Synchronizer =
	    {   id: id,
	        height: -1,
	        peers: peers,
	        peerStatus: peers.mapBy(x => {base:-1, top:-1}),
	        openRequests: Set(),
	        lastSyncedHeight: -1,
	        statusMsgs: Set(),
	        responses: Set(),
	    }

	/// Auxiliary function to iterate over the received status messages
	pure def updatePeerStatus (ps: Address -> SyncStatus, msgs: Set[SyncStatusMsg]) : Address -> SyncStatus =
	    msgs.fold(ps, (newStatus , msg) =>
	        if (newStatus.get(msg.peer).top < msg.top) // TODO: think about base?
	            newStatus.put(msg.peer, {base: msg.base, top: msg.top})
	        else
	            newStatus
	    )

	/// inform the synchronizer that consensus has entered height h
	pure def syncNewHeight (s: Synchronizer, h: Height) : Synchronizer =
	    if (h <= s.height)
	        s
	    else
	        s.with("height", h)

	/// returned by the synchronizer: sync is the new state, so is the output towards
	/// the consensus driver, req are messages sent towards peers/servers
	type ClientResult = {
	    sync: Synchronizer,
	    so: SynchronizerOutput,
	    req: Option[RequestMsg]
	}

	/// We have received a certificate. now we need to issue the
	/// corresponding block request  and generate a certificate output.
	pure def syncHandleCertificate (s: Synchronizer, cert: Set[Vote], peer: str ) : ClientResult =
	    val blockReq = {    client: s.id,
	                        server: peer,
	                        rtype: SyncBlock,
	                        height: s.height}
	    {   sync: {...s, openRequests: Set(blockReq)}, // If we have parallelization we need to be more precise here
	        so: SOCertificate(cert),
	        req: Some(blockReq)}

	/// we have received a block. now we need to  generate a block output
	pure def syncHandleBlock (s: Synchronizer, p: Proposal) : ClientResult =
	    {   sync: {...s,    openRequests: Set(), // If we have parallelization we need to remove one element,
	                        lastSyncedHeight: s.height }, // blockheight,
	        so: SOBlock(p),
	        req: None}

	/// step of a client:
	/// 1. update peer statuses, 2. if there is no open request, request something
	/// 3. otherwise check whether we have a response and act accordingly
	pure def syncClient (s: Synchronizer) : ClientResult =
	    val newPeerStates = updatePeerStatus(s.peerStatus, s.statusMsgs)
	    val newS = { ...s, peerStatus: newPeerStates}
	    if (s.lastSyncedHeight >= s.height)
	        // nothing to do
	        {   sync: newS,
	            so: SONoOutput,
	            req: None}
	    else
	        val goodPeers = s.peers.filter(p => newPeerStates.get(p).base <= s.height  
	                                            and s.height <= newPeerStates.get(p).top )
	        if (goodPeers.size() > 0)
	            if (s.openRequests.size() == 0)
	                // we start the sync "round" by asking for a certificate
	                val req = { client: s.id,
	                            server: goodPeers.fold("", (acc, i) => i), //chooseSome(),
	                            rtype: SyncCertificate,
	                            height: s.height}
	                {   sync: {... newS, openRequests: s.openRequests.union(Set(req))},
	                    so: SONoOutput,
	                    req: Some(req)
	                }
	            else
	                // we issued a request before, let's see whether there is a response
	                if (s.responses.size()> 0)
	                    val resp = s.responses.fold(emptyResponseMsg, (acc, i) => i) //chooseSome() // in the future there might be parallelization
	                   
	                    match resp.response {
	                        | RespBlock(prop) => syncHandleBlock(newS, prop)
	                        | RespCertificate(cert) => syncHandleCertificate(newS, cert, goodPeers.fold("", (s,x) => x))
	                    }
	                else
	                    // I don't have response
	                    // this might be timeout logic
	                    {   sync: newS,
	                        so: SONoOutput,
	                        req: None}
	        else
	            // no peers
	            {   sync: newS,
	                so: SONoOutput,
	                req: None}
}
