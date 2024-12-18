// -*- mode: Bluespec; -*-

module valuesyncServerTest {

  import valuesyncServer.* from "./valuesyncServer"
  import valuesync.* from "./valuesync"

  val s = "v1"
  val c = "v2"

  val chain = List(
    { decision: mkProposal("v3", 0, 0, "value1", -1),
      commit: Set()
    },
    { decision: mkProposal("v2", 1, 1, "value2", 0),
      commit: Set()
    }
  )

  action sendRequestTo(v, req) = all {
    statusBuffer' = statusBuffer,
    requestsBuffer' = requestsBuffer.put(v, requestsBuffer.get(v).union(Set(req))),
    responsesBuffer' = responsesBuffer,
    unchangedServer,
  }

  action clearResponses(client) = all {
    statusBuffer' = statusBuffer,
    requestsBuffer' = requestsBuffer,
    responsesBuffer' = responsesBuffer.put(client, Set()),
    unchangedServer,
  }

  run fullDeliverRequest(client, server, rtype, height) =
    sendRequestTo(server, {
      client: client,
      server: server,
      rtype: rtype,
      height: height,
    })
    .then(deliverRequest(s))
    .then(stepServer(s))

  // A server is created with no state. No responses should be produced.
  // This run can be used as initialization for other tests.
  run valuesyncServerInitTest =
    all {
      initServer(Set(s)),
      initVsync(Set(s, c)),
    }

    .then(broadcastStatus(s))
    // FIXME: I don't think there is a point on broadcast a status message
    // here, since the value store is empty.
    .expect(statusBuffer.get(c).contains({
      peer: s,
      base: 0,
      top: -1,
    }))

    // Requests are all rejected
    .then(fullDeliverRequest(c, s, SyncCertificate, 0))
    .then(fullDeliverRequest(c, s, SyncValue, 0))
    .then(fullDeliverRequest(c, s, SyncValueStoreEntry, 0))
    .expect(vsyncServers.get(s).requestMsgs.size() == 0)
    .expect(responsesBuffer.get(c).size() == 0)

  // The server knows a chain with 1 element (height 0).
  // It replies to request to height 0, but not for height 1.
  run valuesyncServerSingleHeightTest =
    valuesyncServerInitTest

    // Now the server knows height 0
    .then(updateServer(s, chain.slice(0, 1)))

    .then(broadcastStatus(s))
    .expect(statusBuffer.get(c).contains({
      peer: s,
      base: 0,
      top: 0,
    }))

    .then(fullDeliverRequest(c, s, SyncCertificate, 0))
    .expect(responsesBuffer.get(c).contains({
      client: c,
      server: s,
      height: 0, 
      response: RespCertificate(chain[0].commit),
    }))

    .then(fullDeliverRequest(c, s, SyncValue, 0))
    .expect(responsesBuffer.get(c).contains({
      client: c,
      server: s,
      height: 0, 
      response: RespValue(chain[0].decision),
    }))

    .then(fullDeliverRequest(c, s, SyncValueStoreEntry, 0))
    .expect(responsesBuffer.get(c).contains({
      client: c,
      server: s,
      height: 0, 
      response: RespValueStoreEntry(chain[0]),
    }))

    .then(clearResponses(c))

    // Requests for height 2 are ignored
    .then(fullDeliverRequest(c, s, SyncCertificate, 1))
    .then(fullDeliverRequest(c, s, SyncValue, 1))
    .then(fullDeliverRequest(c, s, SyncValueStoreEntry, 1))
    .expect(vsyncServers.get(s).requestMsgs.size() == 0)
    .expect(responsesBuffer.get(c).size() == 0)

  // The server knows a chain with 2 elements (heights 0 and 1).
  // It replies properly to requests to heights 0 and 1.
  run valuesyncServerTwoHeightsTest =
    valuesyncServerInitTest

    // Now the server knows heights 0 and 1
    .then(updateServer(s, chain.slice(0, 2)))

    .then(broadcastStatus(s))
    .expect(statusBuffer.get(c).contains({
      peer: s,
      base: 0,
      top: 1,
    }))

    .then(fullDeliverRequest(c, s, SyncCertificate, 0))
    .expect(responsesBuffer.get(c).contains({
      client: c,
      server: s,
      height: 0, 
      response: RespCertificate(chain[0].commit),
    }))

    .then(fullDeliverRequest(c, s, SyncValue, 1))
    .expect(responsesBuffer.get(c).contains({
      client: c,
      server: s,
      height: 1, 
      response: RespValue(chain[1].decision),
    }))

    .then(fullDeliverRequest(c, s, SyncValueStoreEntry, 0))
    .expect(responsesBuffer.get(c).contains({
      client: c,
      server: s,
      height: 0, 
      response: RespValueStoreEntry(chain[0]),
    }))
}
