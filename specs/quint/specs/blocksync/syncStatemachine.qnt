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
	/// initializes synchronizer
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

	action syncStep(v) = any {
	        syncDeliverReq(v),
	        syncDeliverResp(v),
	        syncDeliverStatus(v),
	    //    syncUpdateServer(v),	// update the server with the latest state of the chain
	        syncStepServer(v),	// looking for a request and sends responses
	        syncStatusStep(v),
	        //syncTimeout(v) // TODO:
	}

	//
	// Complex actions for a validator: consensus and blocksync
	//






}
