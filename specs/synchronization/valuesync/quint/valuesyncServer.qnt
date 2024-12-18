// -*- mode: Bluespec; -*-
//
// ValueSync protocol: server side, replies to client requests.
//

module valuesyncServer {

  import valuesync.* from "./valuesync"

  type Server = {
    id: Address,

    chain: List[ValueStoreEntry],

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
    server: Server,
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
                  | SyncValue  =>
                          val bl = {  client: m.client,
                                      server: s.id,
                                      height: m.height,
                                      response: RespValue(s.chain[m.height].decision)}
                          Some(bl)
                  | SyncValueStoreEntry =>
	                  val bs = {  client: m.client,
	                              server: s.id,
	                              height: m.height,
	                              response: RespValueStoreEntry(s.chain[m.height])}
                          Some(bs)
              }
          else None
      {   server: { ...s, requestMsgs: s.requestMsgs.exclude(Set(m))},
          msg: result}
    else {
      server: s,
      msg: None}

  //
  // State machine
  //

  var vsyncServers: Address -> Server

  //
  // Actions
  //

  action initServer(nodes) = all {
    vsyncServers' = nodes.mapBy(v => newServer(v)),
  }

  action unchangedServer = all {
    vsyncServers' = vsyncServers,
  }

  // Deliver a request message, from requestsBuffer, to node
  action deliverRequest(node) = all {
    requestsBuffer.get(node).size() > 0,
    val server = vsyncServers.get(node)
    nondet msg = requestsBuffer.get(node).oneOf()
    all {
      vsyncServers' = vsyncServers.put(node, {... server,
                                  requestMsgs: server.requestMsgs.union(Set(msg))}),
      statusBuffer' = statusBuffer,
      requestsBuffer' = requestsBuffer.put(node, requestsBuffer.get(node).exclude(Set(msg))),
      responsesBuffer' = responsesBuffer,
    }
  }

  // Server at node broadcasts its value store status
  action broadcastStatus(node) = all {
    val server = vsyncServers.get(node)
    val msg = server.syncStatus()
    all {
      vsyncServers' = vsyncServers.put(node, server),
      statusBuffer' = broadcastStatusMsg(statusBuffer, msg),
      requestsBuffer' = requestsBuffer,
      responsesBuffer' = responsesBuffer,
    }
  }

  // Server at node takes a step (checking for requests and responding)
  action stepServer(node) = all {
    val server = vsyncServers.get(node)
    val result = server.syncServer()
    all {
      vsyncServers' = vsyncServers.put(node, result.server),
      statusBuffer' = statusBuffer,
      requestsBuffer' = requestsBuffer,
      responsesBuffer' = match result.msg {
          | Some(m) => responsesBuffer.sendResponse(m)
          | None => responsesBuffer
      },
    }
  }

  // Updates the server status, the latest available value store content.
  // This action must be called by the component that knows the chain.
  action updateServer(node, chain) = all {
    val server = vsyncServers.get(node)
    all {
      chain != server.chain,
      vsyncServers' = vsyncServers.put(node, { ...server, chain: chain }),
      unchangedVsync,
    }
  }

}
