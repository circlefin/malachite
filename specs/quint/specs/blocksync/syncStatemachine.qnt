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

  // deliver a status message to client v
  action syncDeliverStatus(v) =  all {
     deliverStatus(v),
     unchangedServer,
  }

  // deliver a response to client v
  action syncDeliverResp(v) =  all {
    deliverResponse(v),
    unchangedServer,
  }

  // deliver a request to server v
  action syncDeliverReq(v) =  all {
    deliverRequest(v),
    unchangedClient,
  }

  /// any blocksync step independent of consensus. unchange is an action
  /// that can be used for composition
  action pureSyncStep (v, unchange) = all {
    any {
      syncDeliverReq(v),
      syncDeliverResp(v),
      syncDeliverStatus(v),
      syncStepServer(v),
      syncStatusStep(v),
    },
    unchange
  }


  // Client v takes a step
  action syncStepClient(v, outputAction) = all {
    val result = bsyncClient(bsyncClients.get(v))
    all {
      // updates the client's state
      applyClientUpdate(v, result),
      // This is the part that interacts with the consensus.
      // It can be the driver or the mocked consensus logic.
      outputAction(v, result.so), 
      unchangedServer,
    }
  }





}
