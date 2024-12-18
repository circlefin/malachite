// -*- mode: Bluespec; -*-
//
// Test unit for valuesync client-side protocol.

module valuesyncClientTest {

  import valuesyncClient.* from "./valuesyncClient"
  import valuesync.* from "./valuesync"

  val nodes = Set("v1", "v2", "v3")
  val chain = List(
    { decision: mkProposal("v3", 0, 0, "value1", -1),
      commit: Set()
    },
    { decision: mkProposal("v2", 1, 1, "value2", 0),
      commit: Set()
    }
  )

  var outputs: List[VsyncClientOutput]

  action init = all {
    initClient(nodes),
    initVsync(nodes),
    outputs' = List(),
  }

  action addOutput(v, so) = all {
    outputs' = if (so == SONoOutput) outputs else outputs.append(so),
  }

  action produceStatus(status) = all {
    outputs' = outputs,
    unchangedClient,
    statusBuffer' = broadcastStatusMsg(statusBuffer, status),
    requestsBuffer' = requestsBuffer,
    responsesBuffer' = responsesBuffer,
  }

  action produceResponse(resp) = all {
    outputs' = outputs,
    unchangedClient,
    statusBuffer' = statusBuffer,
    requestsBuffer' = requestsBuffer,
    responsesBuffer' = sendResponse(responsesBuffer, resp),
  }

  // Test clientLogic with "full == false"
  run bsClientBasicTest =
    init

    // Nothing happens until the height is set
    .then(clientLogic("v1", addOutput, false))
    .expect(length(outputs) == 0)
    .expect(vsyncClients.get("v1").lastSyncedHeight == -1)
    .expect(requestsBuffer.keys().forall(x => requestsBuffer.get(x).size() == 0))
    .then(all {
      newHeightClient("v1", 1),
      outputs' = outputs,
      unchangedVsync,
    })
    // We don't react to new height immediately
    .expect(length(outputs) == 0)
    .expect(requestsBuffer.keys().forall(x => requestsBuffer.get(x).size() == 0))
    .expect(vsyncClients.get("v1").lastSyncedHeight == -1)

    // Nothing happens until status are received
    .then(clientLogic("v1", addOutput, false))
    .expect(length(outputs) == 0)
    .expect(requestsBuffer.keys().forall(x => requestsBuffer.get(x).size() == 0))
    .expect(vsyncClients.get("v1").lastSyncedHeight == -1)

    // We receive a status report from v3, but nothing for us
    .then(produceStatus({ peer: "v3", base: 0, top: 0,}))
    .then(all {
      deliverStatus("v1"),
      outputs' = outputs,
    })
    // Nothing happens because the status is bad
    .then(clientLogic("v1", addOutput, false))
    .expect(length(outputs) == 0)
    .expect(requestsBuffer.keys().forall(x => requestsBuffer.get(x).size() == 0))
    .expect(vsyncClients.get("v1").lastSyncedHeight == -1)

    // We receive a status report from v2
    .then(produceStatus({ peer: "v2", base: 0, top: 1,}))
    .then(all {
      deliverStatus("v1"),
      outputs' = outputs,
    })
    // Then we send a certificate request to v2
    .then(clientLogic("v1", addOutput, false))
    .expect(length(outputs) == 0)
    .expect(requestsBuffer.keys().forall(x =>
      if (x != "v2") requestsBuffer.get(x).size() == 0
      else requestsBuffer.get(x).size() == 1))
    .expect(requestsBuffer.get("v2").contains({
      client: "v1",
      server: "v2",
      rtype: SyncCertificate,
      height: 1,
     }))
    .expect(vsyncClients.get("v1").lastSyncedHeight == -1)

    // The server replies with a certificate
    .then(produceResponse({
      client: "v1",
      server: "v2",
      height: 1,
      response: RespCertificate(chain[0].commit),
    }))
    .then(all {
      deliverResponse("v1"),
      outputs' = outputs,
    })

    // Then we: (i) produce a certificate output and
    // (ii) send a value request to v2.
    .then(clientLogic("v1", addOutput, false))
    .expect(length(outputs) == 1)
    .expect(outputs[0] == SOCertificate(chain[0].commit))
    .expect(requestsBuffer.keys().forall(x =>
      if (x != "v2") requestsBuffer.get(x).size() == 0
      else requestsBuffer.get(x).size() == 2))
    .expect(requestsBuffer.get("v2").contains({
      client: "v1",
      server: "v2",
      rtype: SyncValue,
      height: 1,
     }))
    .expect(vsyncClients.get("v1").lastSyncedHeight == -1)

    // The server replies with a value
    .then(produceResponse({
      client: "v1",
      server: "v2",
      height: 1,
      response: RespValue(chain[0].decision),
    }))
    .then(all {
      deliverResponse("v1"),
      outputs' = outputs,
    })

    .then(clientLogic("v1", addOutput, false))
    .expect(length(outputs) == 2)
    .expect(outputs[1] == SOValue(chain[0].decision))
    .expect(vsyncClients.get("v1").lastSyncedHeight == 1)

  // Test clientLogic with "full == true"
  run bsClientBasicFastTest =
    init

    // Nothing happens until the height is set
    .then(clientLogic("v1", addOutput, true))
    .expect(length(outputs) == 0)
    .expect(requestsBuffer.keys().forall(x => requestsBuffer.get(x).size() == 0))
    .then(all {
      newHeightClient("v1", 1),
      outputs' = outputs,
      unchangedVsync,
    })
    .expect(vsyncClients.get("v1").lastSyncedHeight == -1)
    // We don't react to new height immediately
    .expect(length(outputs) == 0)
    .expect(requestsBuffer.keys().forall(x => requestsBuffer.get(x).size() == 0))

    // Nothing happens until status are received
    .then(clientLogic("v1", addOutput, true))
    .expect(length(outputs) == 0)
    .expect(requestsBuffer.keys().forall(x => requestsBuffer.get(x).size() == 0))
    .expect(vsyncClients.get("v1").lastSyncedHeight == -1)

    // We receive a status report from v3, but nothing for us
    .then(produceStatus({ peer: "v3", base: 0, top: 0,}))
    .then(all {
      deliverStatus("v1"),
      outputs' = outputs,
    })
    // Nothing happens because the status is bad
    .then(clientLogic("v1", addOutput, true))
    .expect(length(outputs) == 0)
    .expect(requestsBuffer.keys().forall(x => requestsBuffer.get(x).size() == 0))
    .expect(vsyncClients.get("v1").lastSyncedHeight == -1)

    // We receive a status report from v2
    .then(produceStatus({ peer: "v2", base: 0, top: 1,}))
    .then(all {
      deliverStatus("v1"),
      outputs' = outputs,
    })
    // Then we send a certificate request to v2
    .then(clientLogic("v1", addOutput, true))
    .expect(length(outputs) == 0)
    .expect(vsyncClients.get("v1").lastSyncedHeight == -1)
    .expect(requestsBuffer.keys().forall(x =>
      if (x != "v2") requestsBuffer.get(x).size() == 0
      else requestsBuffer.get(x).size() == 1))
    .expect(requestsBuffer.get("v2").contains({
      client: "v1",
      server: "v2",
      rtype: SyncValueStoreEntry,
      height: 1,
     }))

    // The server replies with a certificate and value
    .then(produceResponse({
      client: "v1",
      server: "v2",
      height: 1,
      response: RespValueStoreEntry(chain[0]),
    }))
    .then(all {
      deliverResponse("v1"),
      outputs' = outputs,
    })
    .expect(vsyncClients.get("v1").lastSyncedHeight == -1)

    // Then we produce two outputs and no message
    .then(clientLogic("v1", addOutput, true))
    .expect(length(outputs) == 1)
    .expect(outputs[0] == SOValueStoreEntry(chain[0]))
    .expect(requestsBuffer.keys().forall(x =>
      if (x != "v2") requestsBuffer.get(x).size() == 0
      else requestsBuffer.get(x).size() == 1))
    .expect(vsyncClients.get("v1").lastSyncedHeight == 1)

}
