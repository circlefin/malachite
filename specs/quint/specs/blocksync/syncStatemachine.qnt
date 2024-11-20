// -*- mode: Bluespec; -*-

module syncStatemachine {

  // General definitions
  import blocksync.* from "./blocksync"

  import bsync_client.* from "./client"
  import bsync_server.* from "./server"

  // ****************************************************************************
  // State machine
  // ****************************************************************************
  //
  // The statemachine is put on top of statemachineAsync, that is, we use its
  // initialization and steps, and add the updates to the variables defined below
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
