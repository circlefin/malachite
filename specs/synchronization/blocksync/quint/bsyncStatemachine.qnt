// -*- mode: Bluespec; -*-
//
// State machine for the block sync protocol.
//

module bsyncStatemachine {

  import blocksync.* from "./blocksync"
  import blocksyncClient.* from "./blocksyncClient"
  import blocksyncServer.* from "./blocksyncServer"

  export blocksync.*
  export blocksyncClient.*
  export blocksyncServer.*

  // ****************************************************************************
  // State machine
  // ****************************************************************************
  //
  // The statemachine is put on top of statemachineAsync, that is, we use its
  // initialization and steps, and add the updates to the variables defined below
  //

  //
  // Properties
  //

  /// A request should only be sent to a server who has reported, via status
  /// message, having data for the requested height.
  // FIXME: this assume no prunning, namely chain.length() == latestHeight.
  val validRequestInvariant = bsyncServers.keys().forall(s =>
    requestsBuffer.get(s).forall(m =>
      m.height <= bsyncServers.get(s).chain.length()
    )
  )


  /// No request for past heights active
  val noOldRequestsInv = bsyncClients.keys().forall(c =>
    bsyncClients.get(c).openRequests.forall(req =>
      req.height >= bsyncClients.get(c).height))



  /// A server only replies to a request received from a client.
  /// The client request might have timed out.
  val serverRespondsToRequestingPeersInvariant = bsyncClients.keys().forall(c =>
    responsesBuffer.get(c).forall(m =>
      bsyncClients.get(c).openRequests.union(expiredTimeouts.get(c))
      .exists(req => matchingMessages(req, m))
    )
  )


  /// A server only replies to a request received from a client.
  /// This witness should report a scenario where a request timeouts, the
  /// client submits a new one, and a late response is received.
  val serverRespondsToRequestingPeersWitness = bsyncClients.keys().forall(c =>
    responsesBuffer.get(c).forall(m =>
      bsyncClients.get(c).openRequests.exists(req =>
        matchingMessages(req, m))
    )
  )

  /// Every request will eventually terminate. 
  /// We cannot currently verify it with Apalache.
  temporal terminationRequest = 
    bsyncClients.keys().forall(c => 
      always (bsyncClients.get(c).openRequests.forall(req =>
        eventually (not(bsyncClients.get(c).openRequests.contains(req))))))

  //
  // Actions
  //

  /// initializing the variables of the sync part of the state machine
  action syncInit(validators) = all {
    initClient(validators),
    initServer(validators),
    initBsync(validators),
  }

  action syncUnchangedAll = all {
    unchangedServer,
    unchangedClient,
    unchangedBsync,
  }

  //
  // Actions for the Environment to send a node to a new height
  //

  /// environment sends the node to the next height.
  /// initializes client
  action newHeightActionSync(v, valset, h) = all {
    newHeightClient(v, h),
    unchangedBsync,
    unchangedServer,
  }

  //
  // Actions for process steps in the sync protocol
  //

  // Server v announces its status
  action syncStatusStep(v) = all {
    all {
      broadcastStatus(v),
      unchangedClient,
    }
  }

  // Server v takes a step (checking for requests and responding)
  action syncStepServer(v) = all {
    all {
      stepServer(v),
      unchangedClient,
    }
  }

  //
  // Actions for the environment to deliver messages in the sync protocol
  // Implemented by the blocksync server or client.
  //

  /// Deliver a status message to client v
  action syncDeliverStatus(v) =  all {
     deliverStatus(v),
     unchangedServer,
  }

  /// Deliver a response to client v
  action syncDeliverResp(v) =  all {
    deliverResponse(v),
    unchangedServer,
  }

  /// Deliver a request to server v
  action syncDeliverReq(v) =  all {
    deliverRequest(v),
    unchangedClient,
  }

  /// Client v runs a protocol step
  action syncStepClient(v, out, full) = all {
    clientLogic(v, out, full),
    unchangedServer,
  }

  /// A client v's request times out
  action syncClientTimeout(v) = all {
    clientTimeout(v),
    unchangedServer,
  }

  /// any blocksync step independent of consensus.
  /// unchange is an action that can be used for composition
  action pureSyncStep (v, unchange) = all {
    any {
      syncDeliverReq(v),
      syncDeliverResp(v),
      syncDeliverStatus(v),
      syncStepServer(v),
      syncStatusStep(v),
      syncClientTimeout(v),
    },
    unchange
  }

}
