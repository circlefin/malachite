// -*- mode: Bluespec; -*-
//
// Blocksync executable model running together with the consensus logic.
// main init action: initAll
// main step action: stepWithBlockSync
//

module bsyncWithConsensus {

  import bsyncStatemachine.* from "./bsyncStatemachine"

  /// Consensus state machine setup
  import statemachineAsync(
    validators = Set("v1", "v2", "v3", "v4"),
    validatorSet = Set("v1", "v2", "v3", "v4").mapBy(x => 1),
    Faulty = Set("v1"),
    Values = Set("red", "blue"),
    Rounds = Set(0, 1, 2, 3),
    Heights = Set(0) // , 1, 2, 3)
  ).* from "./consensus/statemachineAsync"

  //
  // Properties
  //

  /// Witness for some data being synced. It is triggered if there is a client
  /// that gets a syncing output that is different from None. That is, it has
  /// received a certificate, or a block, or both (a block store entry).
  //
  // With the default `init` action, it is unlikely to observe this scenario,
  // even with 200 steps (around 30 minutes) execution:
  // $ quint run --invariant anyClientOutputWitness bsyncWithConsensus.qnt
  // Therefore we have a special init action:
  // $ quint run --invariant anyClientOutputWitness bsyncWithConsensus.qnt --init initSetup --step stepWithBlockSync --maxSteps 60
  // Use --seed=0x1060f6cddc9cb5 to reproduce.

  val anyClientOutputWitness = Correct.forall(p =>
    system.get(p).incomingSyncCertificates == Set())

  //
  // Actions
  //

  /// initialize consensus and block sync
  action initAll = all {
    init,
    syncInit(validators)
  }

  /// Environment sends the node to the next height.
  action newHeightActionAll(v, valset, h) = all {
    system.get(v).es.chain.length() == h, // precondition for calling this
    newHeightActionSync(v, valset, h),
    newHeightAction(v, valset, h),
  }

  /// Update server v from the consensus' blockchain
  action syncUpdateServer(v) = all {
    val chain = system.get(v).es.chain
    all {
      updateServer(v, chain),
      unchangedClient,
      unchangedAll,
    }
  }

  /// Validator step in the consensus protocol, no changes to block sync
  action syncValStep(v) = all {
    valStep(v),
    syncUnchangedAll
  }

  /// Main step function: either a consensus state-machine step or
  /// a block sync protocol step.
  action stepWithBlockSync = any {
    // consensus takes a step
    all {
      step,
      syncUnchangedAll
    },
    // block sync takes a step
    nondet v = oneOf(Correct)
    any {
      pureSyncStep(v, unchangedAll),
      // Block sync client check if there are responses to apply to consensus
      syncStepClient(v, putSyncOutputIntoNode, false),
      syncStepClient(v, putSyncOutputIntoNode, true),
      syncUpdateServer(v),
    },
    // a node goes to next height
    nondet v = oneOf(Correct)
    newHeightActionAll(v, validatorSet, system.get(v).es.chain.length())
  }

  action syncStep =
    nondet v = oneOf(Correct)
    pureSyncStep(v, unchangedAll)

  //
  // Interesting scenarios
  //

  /// auxiliary function for initHeight
  pure def setChain(s: DriverState, c: List[{decision: Proposal, commit: Set[Vote]}]): DriverState =
    {... s, chain: c}

  /// auxiliary function for initHeight
  /// constructs a commit certificate for a height and value
  pure def commitSet (h: Height, v: Value) : Set[Vote] =
    Set("v1", "v2", "v3").map(n => mkVote(Precommit, n, h, 0, v))

  /// An action to set up an initial state with some nodes already decided up
  /// to height h. Sets up an initial state where v4 starts late, and v2 and v3
  /// have already reached height h. v1 is not correct, so not modelled.
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
    .then(syncStepClient("v4", putSyncOutputIntoNode, false)) // ask for certificate
    .then(all{unchangedAll, syncDeliverReq("v2")})
    .then(all{unchangedAll, syncStepServer("v2")})
    .then(all{unchangedAll, syncDeliverResp("v4")})
    .then(syncStepClient("v4", putSyncOutputIntoNode, false)) // ask for block and give certificate to node
    .expect(system.get("v4").incomingSyncCertificates.size() > 0)
    .expect(not(anyClientOutputWitness))
    .then(all{unchangedAll, syncDeliverReq("v2")})
    .then(all{unchangedAll, syncStepServer("v2")})
    .then(all{unchangedAll, syncDeliverResp("v4")})
    .then(syncStepClient("v4", putSyncOutputIntoNode, false))
    .expect(system.get("v4").incomingSyncProposals.size() > 0)
    .then(3.reps(_ => syncValStep("v4")))
    .expect(system.get("v4").es.chain.length() > h)

  run parisRetreat =
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

  /// initSetup setups two servers (v2, v3) at height 4, which broadcast their
  /// status. Client v4 learns the status and starts syncing from height 0.
  action initSetup =
    initHeight(4)
    .then(syncUpdateServer("v2"))
    .then(syncUpdateServer("v3"))
    .then(all{unchangedAll, syncStatusStep("v2")})
    .then(all{unchangedAll, syncStatusStep("v3")})
    .then(all{unchangedAll, syncDeliverStatus("v4")})
    .then(all{unchangedAll, syncDeliverStatus("v4")})
    .then(newHeightActionAll("v4", validatorSet, 0))

  run lausanneRetreat =
    initHeight(2)
    // FIXME: I had to put it here, instead of syncCycle(h)
    // This is not ideal, but it works.
    .then(syncUpdateServer("v2"))
    .then(newHeightActionAll("v4", validatorSet, 0))
    .then(all{unchangedAll, syncStatusStep("v2")})
    .then(all{unchangedAll, syncDeliverStatus("v4")})
    .then(syncStepClient("v4", putSyncOutputIntoNode, false)) // ask for certificate
    // request for certificate is sent to v2
    .expect(requestsBuffer.get("v2").contains({ client: "v4", height: 0, rtype: SyncCertificate, server: "v2" }))
    // v3 wakes up and sends it status to v4
    .then(syncUpdateServer("v3"))
    .then(all{unchangedAll, syncStatusStep("v3")})
    .then(all{unchangedAll, syncDeliverStatus("v4")})
    // v4's request to v2 times out...
    .then(all{unchangedAll, syncClientTimeout("v4")})
    // after handling the timeout a request for certificate is sent to v3
    .expect(requestsBuffer.get("v3").contains({ client: "v4", height: 0, rtype: SyncCertificate, server: "v3" }))

}
