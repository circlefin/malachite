// -*- mode: Bluespec; -*-
//
// Blocksync protocol: client side.

module bsync_client {
  import blocksync.* from "./blocksync"

  /// The state of the synchronizer
  type BsyncClient = {
    id: Address,

    peerStatus: Address -> BlockRange,
    openRequests: Set[RequestMsg],

    height: Height,
    lastSyncedHeight: Height,   // "done" if greater than or equal to height
                                // TODO: we could add buffers for certificates and values
    // inbuffers
    statusMsgs: Set[StatusMsg],
    responseMsgs: Set[ResponseMsg],
  }

  type BsyncClientOutput =
    | SOCertificate(Set[Vote])
    | SOBlock(Proposal)
    | SONoOutput

  //
  // BsyncClient functions
  //

  /// Initialize the synchronizer
  pure def initBsyncClient(id: Address, peers: Set[Address]) : BsyncClient = 
    {
      id: id,
      peerStatus: peers.mapBy(x => {base:-1, top:-1}),
      openRequests: Set(),
      height: -1,
      lastSyncedHeight: -1,
      statusMsgs: Set(),
      responseMsgs: Set(),
    }

  /// Auxiliary function to iterate over the received status messages
  pure def updatePeerStatus (ps: Address -> BlockRange, msgs: Set[StatusMsg]) : Address -> BlockRange =
    msgs.fold(ps, (newStatus , msg) =>
        if (newStatus.get(msg.peer).top < msg.top) // TODO: think about base?
            newStatus.put(msg.peer, {base: msg.base, top: msg.top})
        else
            newStatus
    )

  /// inform the synchronizer that consensus has entered height h
  pure def syncNewHeight (s: BsyncClient, h: Height) : BsyncClient =
    if (h <= s.height)
        s
    else
        s.with("height", h)

  /// returned by the synchronizer: sync is the new state, so is the output towards
  /// the consensus driver, req are messages sent towards peers/servers
  type ClientResult = {
    sync: BsyncClient,
    so: BsyncClientOutput,
    req: Option[RequestMsg]
  }



  /// We have received a certificate. now we need to issue the
  /// corresponding block request  and generate a certificate output.
  pure def syncHandleCertificate (s: BsyncClient, cert: Set[Vote], peer: str ) : ClientResult =
    val blockReq = {    client: s.id,
                        server: peer,
                        rtype: SyncBlock,
                        height: s.height}
    {   sync: {...s, openRequests: Set(blockReq)}, // If we have parallelization we need to be more precise here
        so: SOCertificate(cert),
        req: Some(blockReq)}

  /// we have received a block. now we need to  generate a block output
  pure def syncHandleBlock (s: BsyncClient, p: Proposal) : ClientResult =
      {   sync: {...s,    openRequests: Set(), // If we have parallelization we need to remove one element,
                          lastSyncedHeight: s.height }, // blockheight,
          so: SOBlock(p),
          req: None}

  /// step of a client:
  /// 1. update peer statuses, 2. if there is no open request, request something
  /// 3. otherwise check whether we have a response and act accordingly
  pure def bsyncClient (s: BsyncClient) : ClientResult =
      val newPeerStates = updatePeerStatus(s.peerStatus, s.statusMsgs)
      val newS = { ...s, peerStatus: newPeerStates}
      if (s.lastSyncedHeight >= s.height)
          // nothing to do
          {   sync: newS,
              so: SONoOutput,
              req: None}
      else
          val goodPeers = s.peerStatus.keys().filter(p => newPeerStates.get(p).base <= s.height
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
                  if (s.responseMsgs.size()> 0)
                      val resp = s.responseMsgs.fold(emptyResponseMsg, (acc, i) => i) //chooseSome() // in the future there might be parallelization
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

  // State machine

  var bsyncClients: Address -> BsyncClient

  action initClient(nodes) = all {
    bsyncClients' = nodes.mapBy(v => initBsyncClient(v, nodes.exclude(Set(v)))),
  }

  action unchangedClient = all {
    bsyncClients' = bsyncClients,
  }

  action newHeightClient(node, h) = all {
    bsyncClients' = bsyncClients.put(node, syncNewHeight(bsyncClients.get(node), h)),
  }

  // deliver a status message, from the statusBuffer, to node
  action deliverStatus(node) = all {
    statusBuffer.get(node).size() > 0,
    val client = bsyncClients.get(node)
    nondet msg = statusBuffer.get(node).oneOf()
    all {
      bsyncClients' = bsyncClients.put(node, {... client,
                                  statusMsgs: client.statusMsgs.union(Set(msg))}),
      statusBuffer' = statusBuffer.put(node, statusBuffer.get(node).exclude(Set(msg))),
      requestsBuffer' = requestsBuffer,
      responsesBuffer' = responsesBuffer,
    }
  }

  // deliver a response message, from responsesBuffer, to node
  action deliverResponse(node) = all {
    responsesBuffer.get(node).size() > 0,
    val client = bsyncClients.get(node)
    nondet msg = responsesBuffer.get(node).oneOf()
    all {
      bsyncClients' = bsyncClients.put(node, {... client,
                                  responseMsgs: client.responseMsgs.union(Set(msg))}),
      requestsBuffer' = requestsBuffer,
      responsesBuffer' = responsesBuffer.put(node, responsesBuffer.get(node).exclude(Set(msg))),
      statusBuffer' = statusBuffer,
    }
  }

  action applyClientUpdate(v, result) = all {
    bsyncClients' = bsyncClients.put(v, result.sync),
    statusBuffer' = statusBuffer,
    requestsBuffer' = match result.req {
      | Some(m) => requestsBuffer.put(m.server, requestsBuffer.get(m.server).union(Set(m)))
      | None => requestsBuffer
    },
    responsesBuffer' = responsesBuffer,
  }

}
