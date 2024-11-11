// -*- mode: Bluespec; -*-
// ****************************************************************************
// Server
// ****************************************************************************

module server {
	import driver.* from "../driver"
	import blocksync.* from "./blocksync"

	// TODO: this should be changed. The data from NodeState we rely on
	// should be stored (copied) into the server state. In this way we get
	// rid from the dependency on the driver, rendering the server
	// self-contained and easier to test.
	/// The server state are just incoming requests. The actual data used to respond
	/// is from the NodeState
	type Server = {
	    id: Address,
	    inbuffer : Set[RequestMsg]
	}

	//
	// Server functions
	//

	pure def initServer(addr: Address) : Server = {
		id: addr,
		inbuffer : Set()
	}

	/// look into the node state and generate a status message
	pure def syncStatus (server: Server, s: NodeState) : SyncStatusMsg =
	    // TODO: perhaps we should add to height to the chain entries to capture non-zero bases
	    // TODO: the driver can maintain a copy of them `chain`
	    { peer: server.id , base: 0, top: s.es.chain.length() - 1  }

	/// new server state and response messages to be sent
	type ServerOutput = {
	    sync: Server,
	    msg: Option[ResponseMsg],
	}

	pure def syncServer (s: Server, ns: NodeState) : ServerOutput =
	    // TODO: the driver can maintain a copy of them `chain`
	    if (s.inbuffer.size() > 0)
	        val m = s.inbuffer.fold(emptyReqMsg, (acc, i) => i) // chooseSome() // TODO: fix
	        val result =
	            if (m.height < ns.es.chain.length())
	                match m.rtype {
	                    | SyncCertificate =>
	                            val cm = {  client: m.client,
	                                        server: s.id,
	                                        height: m.height,
	                                        response: RespCertificate(ns.es.chain[m.height].commit)}
	                            Some(cm)
	                    | SyncBlock  =>
	                            val bl = {  client: m.client,
	                                        server: s.id,
	                                        height: m.height,
	                                        response: RespBlock(ns.es.chain[m.height].decision)}
	                            Some(bl)
	                }
	            else None
	        {   sync: { ...s, inbuffer: s.inbuffer.exclude(Set(m))},
	            msg: result}
	    else {
	        sync: s,
	        msg: None}
}
