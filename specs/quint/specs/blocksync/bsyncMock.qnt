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
    outputs' = [],
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
    chains.get(v).length() == h, // precondition for calling this
    newHeightActionSync(v, valset, h),
    unchangedMock,
  }

  /// Update server v from the consensus' blockchain
  /// This abstracts as pull-based mechanism: the server consults the chain state.
  action mockUpdateServer(v) = all {
    all {
      updateServer(v, chains.get(v)),
      unchangedClient,
      unchangedMock,
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
      syncStepClient(v, writeAction),  // is checking if there are responses and check whether it need to requ
      mockUpdateServer(v),
      newHeightActionAll(v, validators, chains.get(v).length()),
      decideMock(v),
   }
}
