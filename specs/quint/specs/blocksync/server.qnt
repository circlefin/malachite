// -*- mode: Bluespec; -*-
// ****************************************************************************
// Server
// ****************************************************************************

module server {
	import driver.* from "../driver"
	import blocksync.* from "./blocksync"

	/// The server state are just incoming requests. The actual data used to respond
	/// is from the NodeState
	type Server = {
	    inbuffer : Set[RequestMsg]
	}

	//
	// Server functions
	//

	pure def initServer = {inbuffer : Set()}

	/// look into the node state and generate a status message
	pure def syncStatus (s: NodeState) : SyncStatusMsg =
	// TODO: perhaps we should add to height to the chain entries to capture non-zero bases
	    { peer: s.es.cs.p , base: 0, top: s.es.chain.length() - 1  }

	/// new server state and response messages to be sent
	type ServerOutput = {
	    sync: Server,
	    msg: Option[ResponseMsg],
	}

	pure def syncServer (s: Server, ns: NodeState) : ServerOutput =
	    if (s.inbuffer.size() > 0)
	        val m = s.inbuffer.fold(emptyReqMsg, (acc, i) => i) // chooseSome() // TODO: fix
	        val result =
	            if (m.height < ns.es.chain.length())
	                match m.rtype {
	                    | SyncCertificate =>
	                            val cm = {  client: m.client,
	                                        server: m.server,
	                                        height: m.height,
	                                        response: RespCertificate(ns.es.chain[m.height].commit)}
	                            Some(cm)
	                    | SyncBlock  =>
	                            val bl = {  client: m.client,
	                                        server: m.server,
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
