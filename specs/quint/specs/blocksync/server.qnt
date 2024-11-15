// -*- mode: Bluespec; -*-
//
// Blocksync protocol: server side.

module bsync_server {
	import blocksync.* from "./blocksync"

	type Server = {
	    id: Address,

	    chain: List[BlockStoreEntry],

	    // Incoming requests
	    requestMsgs: Set[RequestMsg]
	}

	pure def newServer(addr: Address) : Server = {
		id: addr,
		chain: List(),
		requestMsgs: Set()
	}

	// generate a status message
	pure def syncStatus (server: Server) : StatusMsg =
	    // TODO: perhaps we should add to height to the chain entries to capture non-zero bases
	    { peer: server.id , base: 0, top: server.chain.length() - 1 }

	/// new server state and response messages to be sent
	type ServerOutput = {
	    sync: Server,
	    msg: Option[ResponseMsg],
	}

	// main method: respond to incoming request, if any
	pure def syncServer (s: Server) : ServerOutput =
	    if (s.requestMsgs.size() > 0)
	        val m = s.requestMsgs.fold(emptyReqMsg, (acc, i) => i) // chooseSome() // TODO: fix
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
	        {   sync: { ...s, requestMsgs: s.requestMsgs.exclude(Set(m))},
	            msg: result}
	    else {
	        sync: s,
	        msg: None}

	// State machine

	var bsyncServers: Address -> Server

	action initServer(nodes) = all {
	    bsyncServers' = nodes.mapBy(v => newServer(v)),
	}

	action unchangedServer = all {
	    bsyncServers' = bsyncServers,
	}

	// workaround to update the server state
	action updateServer(node, server) = all {
	    bsyncServers' = bsyncServers.put(node, server)
	}

	// deliver a request message to node
	action deliverRequest(node, msg) = all {
	    val server = bsyncServers.get(node)
	    bsyncServers' = bsyncServers.put(node, {... server,
	                                requestMsgs: server.requestMsgs.union(Set(msg))}),
	}

}
