// -*- mode: Bluespec; -*-
//
// ValueSync protocol: client side that requests and downloads decisions.
//

module valuesyncClient {

  import valuesync.* from "./valuesync"

  /// The state of the synchronizer
  type VsyncClient = {
    id: Address,

    peerStatus: Address -> ValueRange,
    openRequests: Set[RequestMsg],

    height: Height,
    lastSyncedHeight: Height,   // "done" if greater than or equal to height
                                // TODO: we could add buffers for certificates and values
    // inbuffers
    statusMsgs: Set[StatusMsg],
    responseMsgs: Set[ResponseMsg],
  }

  type VsyncClientOutput =
    | SOCertificate(Set[Vote])
    | SOValue(Proposal)
    | SOValueStoreEntry(ValueStoreEntry)
    | SONoOutput

  //
  // VsyncClient functions
  //

  /// Initialize the synchronizer
  pure def initVsyncClient(id: Address, peers: Set[Address]) : VsyncClient =
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
  pure def updatePeerStatus (ps: Address -> ValueRange, msgs: Set[StatusMsg]) : Address -> ValueRange =
    msgs.fold(ps, (newStatus , msg) =>
      if (newStatus.get(msg.peer).top < msg.top) // TODO: think about base?
        newStatus.put(msg.peer, {base: msg.base, top: msg.top})
      else
        newStatus
    )

  /// inform the synchronizer that consensus has entered height h
  pure def syncNewHeight (s: VsyncClient, h: Height) : VsyncClient =
    if (h <= s.height)
      s
    else
      { ... s,  height: h, 
        openRequests: s.openRequests.filter(req => req.height >= h)}

  /// returned by the synchronizer: sync is the new state, so is the output towards
  /// the consensus driver, req are messages sent towards peers/servers
  type ClientResult = {
    sync: VsyncClient,
    so: VsyncClientOutput,
    req: Option[RequestMsg]
  }

  /// We have received a certificate. now we need to issue the
  /// corresponding value request and generate a certificate output.
  pure def syncHandleCertificate (s: VsyncClient, cert: Set[Vote], peer: str ) : ClientResult =
    val valueReq = {client: s.id,
                    server: peer,
                    rtype: SyncValue,
                    height: s.height}
    { sync: {...s, openRequests: Set(valueReq)}, // If we have parallelization we need to be more precise here
      so: SOCertificate(cert),
      req: Some(valueReq)}

  /// we have received a value. now we need to generate a value output
  pure def syncHandleValue (s: VsyncClient, p: Proposal) : ClientResult =
    { sync: {...s, openRequests: Set(), // If we have parallelization we need to remove one element,
              lastSyncedHeight: s.height }, // value height,
      so: SOValue(p),
      req: None}

  /// we have received a value store entry (value and certificate).
  /// now we need to generate a value output
  pure def syncHandleValueStoreEntry (s: VsyncClient, b: ValueStoreEntry) : ClientResult =
    { sync: {...s, openRequests: Set(), // If we have parallelization we need to remove one element,
               lastSyncedHeight: s.height }, // value height,
      so: SOValueStoreEntry(b),
      req: None}

  /// step of a client:
  /// 1. update peer statuses, 2. if there is no open request, request something
  /// 3. otherwise check whether we have a response and act accordingly
  pure def vsyncClientLogic (s: VsyncClient, fullEntries: bool) : ClientResult =
      val newPeerStates = updatePeerStatus(s.peerStatus, s.statusMsgs)
      val newS = { ...s, peerStatus: newPeerStates}
      if (s.lastSyncedHeight >= s.height)
        // nothing to do
        { sync: newS,
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
                            rtype:  if (fullEntries)
                                      SyncValueStoreEntry
                                    else
                                      SyncCertificate,
                            height: s.height}
                {   sync: {... newS, openRequests: s.openRequests.union(Set(req))},
                    so: SONoOutput,
                    req: Some(req)
                }
            else
                // we issued a request before, let's see whether there is a response
                if (s.responseMsgs.size()> 0)
                    val resp = s.responseMsgs.fold(emptyResponseMsg, (acc, i) => i) //chooseSome() // in the future there might be parallelization
                    val ns = {... newS, responseMsgs: newS.responseMsgs.exclude(Set(resp))}
                    match resp.response {
                        | RespValue(prop) => syncHandleValue(newS, prop)
                        | RespCertificate(cert) => syncHandleCertificate(newS, cert, goodPeers.fold("", (s,x) => x))
                        | RespValueStoreEntry(b) => syncHandleValueStoreEntry(ns, b)
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

  /// step of a client upon timeout: We send the same request to a different server
  // FIXME: what to do if goodPeers is empty?
  pure def vsyncClientTimeout(s: VsyncClient, toMsg: RequestMsg) : ClientResult =
    val newPeerStates = updatePeerStatus(s.peerStatus, s.statusMsgs)

    val goodPeers = s.peerStatus.keys().filter(p => newPeerStates.get(p).base <= s.height
                                            and s.height <= newPeerStates.get(p).top ).
                                            exclude(Set(toMsg.server))
    if (goodPeers.size() > 0)
      val req = {... toMsg, server: goodPeers.fold("", (acc, i) => i)}
      val newS = { ...s,  peerStatus: newPeerStates,
                        openRequests: s.openRequests.exclude(Set(toMsg))
                                                    .union(Set(req))}
      { sync: newS,
        so: SONoOutput,
        req: Some(req)}
    else
      { sync:  { ...s,  peerStatus: newPeerStates,
                        openRequests: s.openRequests.exclude(Set(toMsg))},
        so: SONoOutput,
        req: None}


  // State machine

  var vsyncClients: Address -> VsyncClient
  var expiredTimeouts: Address -> Set[RequestMsg] // to check invariants we need to store past requests

  action initClient(nodes) = all {
    vsyncClients' = nodes.mapBy(v => initVsyncClient(v, nodes.exclude(Set(v)))),
    expiredTimeouts' = nodes.mapBy(v => Set()),
  }

  action unchangedClient = all {
    vsyncClients' = vsyncClients,
    expiredTimeouts' = expiredTimeouts
  }

  action newHeightClient(node, h) = all {
    vsyncClients' = vsyncClients.put(node, syncNewHeight(vsyncClients.get(node), h)),
    expiredTimeouts' = expiredTimeouts,
  }

  // deliver a status message, from the statusBuffer, to node
  action deliverStatus(node) = all {
    statusBuffer.get(node).size() > 0,
    val client = vsyncClients.get(node)
    nondet msg = statusBuffer.get(node).oneOf()
    all {
      vsyncClients' = vsyncClients.put(node, {... client,
                                  statusMsgs: client.statusMsgs.union(Set(msg))}),
      statusBuffer' = statusBuffer.put(node, statusBuffer.get(node).exclude(Set(msg))),
      requestsBuffer' = requestsBuffer,
      responsesBuffer' = responsesBuffer,
      expiredTimeouts' = expiredTimeouts,
    }
  }


  // deliver a response message, from responsesBuffer, to node
  action deliverResponse(node) = all {
    responsesBuffer.get(node).size() > 0,
    val client = vsyncClients.get(node)
    nondet msg = responsesBuffer.get(node).oneOf()
    all {
      vsyncClients' = vsyncClients.put(node, {... client,
                                  responseMsgs: client.responseMsgs.union(Set(msg))}),
      requestsBuffer' = requestsBuffer,
      responsesBuffer' = responsesBuffer.put(node, responsesBuffer.get(node).exclude(Set(msg))),
      statusBuffer' = statusBuffer,
      expiredTimeouts' = expiredTimeouts,
    }
  }

  action applyClientUpdate(v, result) = all {
    vsyncClients' = vsyncClients.put(v, result.sync),
    statusBuffer' = statusBuffer,
    requestsBuffer' = match result.req {
      | Some(m) => requestsBuffer.put(m.server, requestsBuffer.get(m.server).union(Set(m)))
      | None => requestsBuffer
    },
    responsesBuffer' = responsesBuffer,
    expiredTimeouts' = expiredTimeouts,
  }

  // Client v takes a step
  action clientLogic(v, outputAction, full) = all {
    val result = vsyncClientLogic(vsyncClients.get(v), full)
    all {
      // updates the client's state
      applyClientUpdate(v, result),
      // This is the part that interacts with the consensus.
      // It can be the driver or the mocked consensus logic.
      outputAction(v, result.so),
    }
  }


  action clientTimeout(node) = all {
    vsyncClients.get(node).openRequests.size() > 0,
    nondet toMsg = vsyncClients.get(node).openRequests.oneOf()
    val result = vsyncClientTimeout(vsyncClients.get(node), toMsg)
    all {
      vsyncClients' = vsyncClients.put(node, result.sync),
      expiredTimeouts' = expiredTimeouts.put(node, expiredTimeouts.get(node).union(Set(toMsg))),
      requestsBuffer' = match result.req {
            | Some(m) => requestsBuffer.put(m.server, requestsBuffer.get(m.server).union(Set(m)))
            | None => requestsBuffer
      },
    },
    statusBuffer' = statusBuffer,
    responsesBuffer' = responsesBuffer,
  }

}
