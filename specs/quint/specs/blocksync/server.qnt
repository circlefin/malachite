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

	// Deliver a request message, from requestsBuffer, to node
	action deliverRequest(node) = all {
	    requestsBuffer.get(node).size() > 0,
	    val server = bsyncServers.get(node)
	    nondet msg = requestsBuffer.get(node).oneOf()
	    all {
		bsyncServers' = bsyncServers.put(node, {... server,
	                                requestMsgs: server.requestMsgs.union(Set(msg))}),
		statusBuffer' = statusBuffer,
		requestsBuffer' = requestsBuffer.put(node, requestsBuffer.get(node).exclude(Set(msg))),
		responsesBuffer' = responsesBuffer,
	   }
	}

	// Server at node broadcasts its blockchain status
	action broadcastStatus(node) = all {
	    val server = bsyncServers.get(node)
	    val msg = server.syncStatus()
	    all {
		bsyncServers' = bsyncServers.put(node, server),
	        statusBuffer' = broadcastStatusMsg(statusBuffer, msg),
	        requestsBuffer' = requestsBuffer,
	        responsesBuffer' = responsesBuffer,
	    }
	}

	// Server at node takes a step (checking for requests and responding)
	action stepServer(node) = all {
	    val server = bsyncServers.get(node)
	    val result = server.syncServer()
	    all {
		bsyncServers' = bsyncServers.put(node, server),
	        statusBuffer' = statusBuffer,
	        requestsBuffer' = requestsBuffer,
	        responsesBuffer' = match result.msg {
	            | Some(m) => responsesBuffer.sendResponse(m)
	            | None => responsesBuffer
	        },
	    }
	}

	// Updates the server status, the latest available blockchain content.
	// This action must be called by the component that knows the chain.
	action updateServer(node, chain) = all {
	    val server = bsyncServers.get(node)
	    all {
	        chain != server.chain,
	        bsyncServers' = bsyncServers.put(node, { ...server, chain: chain }),
		unchangedBsync,
	    }
	}

}
