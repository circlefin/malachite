// -*- mode: Bluespec; -*-

module blocksyncServerTest {

  import blocksyncServer.* from "./blocksyncServer"
  import blocksync.* from "./blocksync"

  val s = "v1"
  val c = "v2"

  val chain = List(
    { decision: mkProposal("v3", 0, 0, "block1", -1),
      commit: Set()
    },
    { decision: mkProposal("v2", 1, 1, "block2", 0),
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

  run blocksyncServerInitTest =
    all {
      initServer(Set(s)),
      initBsync(Set(s, c)),
    }

    .then(broadcastStatus(s))
    // FIXME: I don't think there is a point on broadcast a status message
    // here, since the blockchain is empty.
    .expect(statusBuffer.get(c).contains({
      peer: s,
      base: 0,
      top: -1,
    }))

    // Requests are all rejected
    .then(fullDeliverRequest(c, s, SyncCertificate, 0))
    .then(fullDeliverRequest(c, s, SyncBlock, 0))
    .then(fullDeliverRequest(c, s, SyncBlockStoreEntry, 0))
    .expect(bsyncServers.get(s).requestMsgs.size() == 0)
    .expect(responsesBuffer.get(c).size() == 0)

  run blocksyncServerSingleHeightTest =
    blocksyncServerInitTest

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

    .then(fullDeliverRequest(c, s, SyncBlock, 0))
    .expect(responsesBuffer.get(c).contains({
      client: c,
      server: s,
      height: 0, 
      response: RespBlock(chain[0].decision),
    }))

    .then(fullDeliverRequest(c, s, SyncBlockStoreEntry, 0))
    .expect(responsesBuffer.get(c).contains({
      client: c,
      server: s,
      height: 0, 
      response: RespBlockStoreEntry(chain[0]),
    }))

    .then(clearResponses(c))

    // Requests for height 2 are ignored
    .then(fullDeliverRequest(c, s, SyncCertificate, 1))
    .then(fullDeliverRequest(c, s, SyncBlock, 1))
    .then(fullDeliverRequest(c, s, SyncBlockStoreEntry, 1))
    .expect(bsyncServers.get(s).requestMsgs.size() == 0)
    .expect(responsesBuffer.get(c).size() == 0)

  run blocksyncServerTwoHeightsTest =
    blocksyncServerInitTest

    // Now the server knows height 0
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

    .then(fullDeliverRequest(c, s, SyncBlock, 1))
    .expect(responsesBuffer.get(c).contains({
      client: c,
      server: s,
      height: 1, 
      response: RespBlock(chain[1].decision),
    }))

    .then(fullDeliverRequest(c, s, SyncBlockStoreEntry, 0))
    .expect(responsesBuffer.get(c).contains({
      client: c,
      server: s,
      height: 0, 
      response: RespBlockStoreEntry(chain[0]),
    }))
}
