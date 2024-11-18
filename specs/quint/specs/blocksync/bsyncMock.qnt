// -*- mode: Bluespec; -*-

module bsyncMock {
  import blocksync.* from "./blocksync"
	import bsync_client.* from "./client"
	import bsync_server.* from "./server"

  import syncStatemachine.* from "./syncStatemachine"

  val validators = Set("v1", "v2", "v3", "v4")
  val Correct = Set("v2", "v3", "v4")

  type Output = {
	  v : Address,
	  out: BsyncClientOutput
  }

  var outputs: List[Output]
  var chains: Address -> List[BlockStoreEntry]

	/// initialize consensus and synchronizer
	action initMockedConsensus = all {
	    // todo
      outputs' = [],
      chains' = Correct.mapBy(_ => []),
	    syncInit(validators)
	}

  action unchangedMock = all {
      outputs' = outputs,
      chains' = chains,   
  }

  action decideMock(v) = all {
    outputs' = outputs,
    chains' = chains.put(v, chains.get(v).append({
      decision: mkProposal("", chains.get(v).length(), 0, "", -1),
      commit: Set(),
    })),
    syncUnchangedAll,
  }

	/// environment sends the node to the next height.
	action newHeightActionAll(v, valset, h) = all {
			chains.get(v).length() == h, // precondition for calling this
	    newHeightActionSync(v, valset, h),
      unchangedMock,
	}

	// Update server v from the consensus' blockchain
	action mockUpdateServer(v) = all {
	    all {
					updateServer(v, chains.get(v)),
	        unchangedClient,
          unchangedMock,
	    }
	}

	// Client v takes a step
	//
	// FIXME: here we need to apply the client output to the consensus
	// state machine. Refactoring this method is much more complex.
	//
	action syncStepClient(v, outputAction) = all {
	    val result = bsyncClient(bsyncClients.get(v))
	    all {
					// TODO: this block should be implemented in client.qnt
					bsyncClients' = bsyncClients.put(v, result.sync),
	        statusBuffer' = statusBuffer,
	        requestsBuffer' = match result.req {
	            | Some(m) => requestsBuffer.put(m.server, requestsBuffer.get(m.server).union(Set(m)))
	            | None => requestsBuffer
	        },
	        responsesBuffer' = responsesBuffer,

	        unchangedServer,

					// This is the part that interacts with the consensus state
					// machine. Moreover, the logic's code should be here, but due
					// to Quint issues we need to keep it in stateMachineAsync.
	        outputAction(v, result.so),
	    }
	}

  action writeAction(v, so) = all {
    outputs' = outputs.append({v: v, out: so}),
    chains' = chains,
  }

  action mockStep(v, act) = all {
    act(v),
    unchangedMock
  }

  action mockStepClient(v) =
    syncStepClient(v, writeAction)

  /// For simple repl evaluation
	action bsyncStep (v, act) = all {
		act(v),
		unchangedMock
	}




	/// main step function: either a consensus state-machine step or a sync protocol step
	action stepMockedConsensus =
    nondet v = oneOf(Correct)
    any{
			syncStepNode(v, unchangedMock),
			// consensus-specific steps
			mockStepClient(v),	// is checking if there are responses and check whether it need to requ
			mockUpdateServer(v),
      newHeightActionAll(v, validators, chains.get(v).length()),
      decideMock(v),
    }
	}
