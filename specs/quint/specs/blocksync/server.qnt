// -*- mode: Bluespec; -*-
// ****************************************************************************
// Server
// ****************************************************************************

module server {
	import blocksync.* from "./blocksync"

	type Server = {
	    id: Address,
	    chain: List[BlockStoreEntry],

	    // Incoming requests
	    inbuffer : Set[RequestMsg]
	}

	pure def initServer(addr: Address) : Server = {
		id: addr,
		chain: List(),
		inbuffer : Set()
	}

	//
	// Server functions
	//

	// generate a status message
	pure def syncStatus (server: Server) : SyncStatusMsg =
	    // TODO: perhaps we should add to height to the chain entries to capture non-zero bases
	    { peer: server.id , base: 0, top: server.chain.length() - 1 }

	// update the server state from the node state
	// at the moment this means updating the server's copy of the chain
	pure def updateServer(s: Server, chain: List[BlockStoreEntry]) : Server = {
	    if (chain.length() > s.chain.length())
		    { ...s, chain: chain }
	    else // FIXME: can it ever happen?
		s
	}

	/// new server state and response messages to be sent
	type ServerOutput = {
	    sync: Server,
	    msg: Option[ResponseMsg],
	}

	// main method: respond to incoming request, if any
	pure def syncServer (s: Server) : ServerOutput =
	    if (s.inbuffer.size() > 0)
	        val m = s.inbuffer.fold(emptyReqMsg, (acc, i) => i) // chooseSome() // TODO: fix
	        val result =
	            if (m.height < s.chain.length())
	                match m.rtype {
	                    | SyncCertificate =>
	                            val cm = {  client: m.client,
	                                        server: s.id,
	                                        height: m.height,
	                                        response: RespCertificate(s.chain[m.height].commit)}
	                            Some(cm)
	                    | SyncBlock  =>
	                            val bl = {  client: m.client,
	                                        server: s.id,
	                                        height: m.height,
	                                        response: RespBlock(s.chain[m.height].decision)}
	                            Some(bl)
	                }
	            else None
	        {   sync: { ...s, inbuffer: s.inbuffer.exclude(Set(m))},
	            msg: result}
	    else {
	        sync: s,
	        msg: None}
}
