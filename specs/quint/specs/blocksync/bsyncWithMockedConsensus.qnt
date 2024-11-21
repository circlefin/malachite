// -*- mode: Bluespec; -*-
//
// Blocksync executable model running with a mocked consensus logic.
//

module bsyncWithMockedConsensus {

  import bsyncStatemachine.* from "./bsyncStatemachine"

  val validators = Set("v1", "v2", "v3", "v4")
  val Correct = Set("v2", "v3", "v4")
  val validatorSet = Set("v1", "v2", "v3", "v4").mapBy(x => 1)

  // State machine

  var outputs: Address -> List[BsyncClientOutput]
  var chains: Address -> List[BlockStoreEntry]

  /// initialize consensus and synchronizer
  action initMockedConsensus = all {
    outputs' = validators.mapBy(_ => []),
    chains' = Correct.mapBy(_ => []),
    syncInit(validators)
  }

  action unchangedMock = all {
    outputs' = outputs,
    chains' = chains,
  }

  /// Consensus mocked logic decides the latest height.
  /// The blockchain increases in size.
  /// The server becomes aware of that once `updateServer` is invoked.
  /// The client becomes aware of that once `newHeightAction` is invoked.
  /// This is an abstract version. We just add a (mocked) block with the right
  /// height on top of the chain. Because of this, implicitly all nodes will
  /// decide on the same block
  action decideMock(v) = all {
    bsyncClients.get(v).height == chains.get(v).length(),
    outputs' = outputs,
    chains' = chains.put(v, chains.get(v).append({
      decision: mkProposal("", chains.get(v).length(), 0, "", -1),
      commit: Set(),
    })),
    syncUnchangedAll,
  }

  /// Environment sends the node to the next height.
  /// This implicitly requires `decideMock(v)` to be previous executed.
  action newHeightActionAll(v, valset, h) = all {
    //chains.get(v).length() == h, // precondition for calling this
    newHeightActionSync(v, valset, h),
    unchangedMock,
  }

  /// Update server v from the consensus' blockchain
  /// This abstracts as pull-based mechanism: the server consults the chain state.
  action syncUpdateServer(v) = all {
    all {
      updateServer(v, chains.get(v)),
      unchangedClient,
      unchangedMock,
    }
  }

  action writeAction(v, so) = all {
    val updateOutput = outputs.get(v).append(so)
    outputs' = outputs.put(v, updateOutput),
    chains' = chains,
  }

  action mockStep(v, act) = all {
    act(v),
    unchangedMock
  }

  /// For simple repl evaluation
  action bsyncStep (v, act) = all {
    act(v),
    unchangedMock
  }

  /// main step function: either a consensus state-machine step or a sync protocol step
  action stepMockedConsensus =
    nondet v = oneOf(Correct)
    any {
      pureSyncStep(v, unchangedMock),
      // consensus-specific steps
      syncStepClient(v, writeAction, false),
      syncUpdateServer(v),
      newHeightActionAll(v, validators, chains.get(v).length()),
      decideMock(v),
   }


  /// An action to set up an initial state with some nodes already decided up to height h
  /// this sets up an initial state where v4 starts late, and v2 and v3 have reached
  /// height h
  action initHeight(h) = all {
    val special = "v4" // TODO proper selection from correct set
    chains' = Correct.mapBy(v => range(0, h).foldl(List(), (acc, i) => acc.append(
        { decision: mkProposal( "", i, 0, "", 0),
          commit: Set() } ))
    ),
    outputs' = validators.mapBy(_ => []),
    syncInit(validators)
   }

  run lausanneRetreat =
    initHeight(2)
    .then(syncUpdateServer("v2"))
    .then(newHeightActionAll("v4", validatorSet, 0))
    .then(all{unchangedMock, syncStatusStep("v2")})
    .then(all{unchangedMock, syncDeliverStatus("v4")})
    .then(syncStepClient("v4", writeAction, false)) // ask for certificate
    // request for certificate is sent to v2
    .expect(requestsBuffer.get("v2").contains({ client: "v4", height: 0, rtype: SyncCertificate, server: "v2" }))
    // v3 wakes up and sends it status to v4
    .then(syncUpdateServer("v3"))
    .then(all{unchangedMock, syncStatusStep("v3")})
    .then(all{unchangedMock, syncDeliverStatus("v4")})
    // v4's request to v2 times out...
    .then(all{unchangedMock, syncClientTimeout("v4")})
    // after handling the timeout a request for certificate is sent to v3
    .expect(requestsBuffer.get("v3").contains({ client: "v4", height: 0, rtype: SyncCertificate, server: "v3" }))

}
