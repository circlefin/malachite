module bsyncWithConsensus {
  
  import blocksync.* from "./blocksync"
	import bsync_client.* from "./client"
	import bsync_server.* from "./server"

	import statemachineAsync(
	    validators = Set("v1", "v2", "v3", "v4"),
	    validatorSet = Set("v1", "v2", "v3", "v4").mapBy(x => 1),
	    Faulty = Set("v1"),
	    Values = Set("red", "blue"),
	    Rounds = Set(0, 1, 2, 3),
	    Heights = Set(0) // , 1, 2, 3)
	    ).* from "../statemachineAsync"

  import syncStatemachine.* from "./syncStatemachine"
  
  
	/// initialize consensus and synchronizer
	action initAll = all {
	    init,
	    syncInit(validators)
	}

	/// environment sends the node to the next height.
	action newHeightActionAll(v, valset, h) = all {
	    newHeightActionSync(v, valset, h),
	    newHeightAction(v, valset, h),
	}

	// Update server v from the consensus' blockchain
	action syncUpdateServer(v) = all {
	    val chain = system.get(v).es.chain
	    all {
					updateServer(v, chain),
	        unchangedClient,
	        unchangedAll,
	    }
	}

	// Client v takes a step
	//
	// FIXME: here we need to apply the client output to the consensus
	// state machine. Refactoring this method is much more complex.
	//
	action syncStepClient(v) = all {
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
	        putSyncOutputIntoNode(v, result.so),
	    }
	}



	// validator step in the system with sync protocol
	action syncValStep(v) = all {
	    valStep(v),
	    syncUnchangedAll
	}



	/// main step function: either a consensus state-machine step or a sync protocol step
	action stepWithBlockSync = any {
			// consensus takes a step
	    all {
	        step,
	        syncUnchangedAll
	    },
	    all{
				unchangedAll,
				nondet v = oneOf(Correct)
				syncStep(v)
				},
			// consensus-specific steps
			nondet v = oneOf(Correct)
			syncStepClient(v),	// is checking if there are responses and check whether it need to requ
			nondet v = oneOf(Correct)
			syncUpdateServer(v)
	}

	//
	// Interesting scenarios
	//

	/// auxiliary function for initHeight
	/// sets the chain
	pure def setChain(s: DriverState, c: List[{decision: Proposal, commit: Set[Vote]}]): DriverState =
	    {... s, chain: c}

	/// auxiliary function for initHeight
	/// constructs a commit certificate for a height and value
	pure def commitSet (h: Height, v: Value) : Set[Vote] =
	    Set("v1", "v2", "v3").map(n => mkVote(Precommit, n, h, 0, v))

	/// An action to set up an initial state with some nodes already decided up to height h
	/// this sets up an initial state where v4 starts late, and v2 and v3 have reached
	/// height h
	action initHeight(h) = all {
	    val special = "v4" // TODO proper selection from correct set
	    val initsystem = Correct.mapBy(v =>
	        // hack
	        if (v == special) initNode(v, validatorSet, 0)
	        else initNode(v, validatorSet, h))
	    nondet decisionValMap = 0.to(h-1).setOfMaps(Values).oneOf()
	    val propMap =  decisionValMap.keys().mapBy(i =>
	                    mkProposal( proposer(validatorSet, i,0),
	                                i,
	                                0,
	                                decisionValMap.get(i),
	                                0))
	    val list = range(0, h).foldl(List(), (acc, i) => acc.append(propMap.get(i)))
	    val chain = list.foldl(List(), (acc, i) =>  acc.append({decision: i, commit: commitSet(i.height, Val(i.proposal))}))
	    all {
	        system' = initsystem.keys().mapBy(x =>
	        // hack
	        if (x == special) initsystem.get(x)
	        else {... initsystem.get(x), es: setChain(initsystem.get(x).es, chain) }),
	            propBuffer' = Correct.mapBy(v => Set()),
	            voteBuffer' = Correct.mapBy(v => Set()),
	            certBuffer' = Correct.mapBy(v => Set()),
	            _hist' = { validator: "INIT", input: NoDInput, output: NoConsensusOutput },
							syncInit(validators)
	    }
	}







	/// a simple scenario where v4 starts height h
	run syncCycle(h) =
	    newHeightActionAll("v4", validatorSet, h)
	    .then(all{unchangedAll, syncStatusStep("v2")})
	    .then(all{unchangedAll, syncDeliverStatus("v4")})
	    .then(syncStepClient("v4")) // ask for certificate
	    .then(all{unchangedAll, syncDeliverReq("v2")})
	    .then(all{unchangedAll, syncStepServer("v2")})
	    .then(all{unchangedAll, syncDeliverResp("v4")})
	    .then(syncStepClient("v4")) // ask for block and give certificate to node
	    .expect(system.get("v4").incomingSyncCertificates.size() > 0)
	    .then(all{unchangedAll, syncDeliverReq("v2")})
	    .then(all{unchangedAll, syncStepServer("v2")})
	    .then(all{unchangedAll, syncDeliverResp("v4")})
	    .then(syncStepClient("v4"))
	    .expect(system.get("v4").incomingSyncProposals.size() > 0)
	    .then(3.reps(_ => syncValStep("v4")))
	    .expect(system.get("v4").es.chain.length() > h)

	run retreat =
	    nondet heightToReach = 1.to(4).oneOf()
	    initHeight( q::debug ("Height:", heightToReach))
	    // FIXME: I had to put it here, instead of syncCycle(h)
	    // This is not ideal, but it works.
            .then(syncUpdateServer("v2"))
	    .then(heightToReach.reps(i => syncCycle(i)))
	    .expect(system.get("v4").es.chain == system.get("v2").es.chain)
	    .then(newHeightActionAll("v4", validatorSet, heightToReach))
	    .expect(system.get("v4").es.cs.height == system.get("v2").es.cs.height)
	    // and now v4 has synced !


  }