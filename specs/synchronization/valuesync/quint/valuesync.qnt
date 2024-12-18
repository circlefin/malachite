// -*- mode: Bluespec; -*-
//
// General definitions for valuesync protocol and state machine for
// the client-server communication, namely, the network.
//

module valuesync {

  import types.* from "./consensus/types"
  export types.*

  type Option[a] =
    | Some(a)
    | None

  type StatusMsg = {
    peer: Address,
    base: Height,
    top: Height
  }

  type ValueRange = {
    base: Height,
    top: Height
  }

  type ReqType =
    | SyncCertificate
    | SyncValue
    | SyncValueStoreEntry

  type RequestMsg = {
    client: Address,
    server: Address,
    rtype: ReqType,
    height: Height
  }

  pure def emptyReqMsg = {
    client: "",
    server: "",
    rtype: SyncCertificate,
    height: -1
  }

  type Response =
    | RespValue(Proposal)
    | RespCertificate(Set[Vote])
    | RespValueStoreEntry(ValueStoreEntry)

  type ResponseMsg = {
    client: Address,
    server: Address,
    height: Height,
    response: Response,
  }

  pure def matchingMessages(req: RequestMsg, resp: ResponseMsg): bool =
    and {
      req.client == resp.client,
      req.server == resp.server,
      req.height == resp.height,
    }


  pure val emptyResponseMsg = {
    client: "",
    server: "",
    height: -1,
    response: RespValue(emptyProposal),
  }

  // *************
  // State machine
  // *************

  /// Messages exchanged by nodes (clients and servers)
  var statusBuffer : Address -> Set[StatusMsg]
  var requestsBuffer : Address -> Set[RequestMsg]
  var responsesBuffer : Address -> Set[ResponseMsg]

  // Auxiliary functions for sending messages

  /// Put the status message in the buffer of every client.
  pure def broadcastStatusMsg(buffer: Address -> Set[StatusMsg], sm: StatusMsg): Address -> Set[StatusMsg] =
    buffer.keys().mapBy(x =>
      if (x == sm.peer) // don't send to yourself
        buffer.get(x)
      else
        { ...buffer.get(x).union(Set(sm)) }
    )

  /// Put the response message in the buffer of the requesting client.
  pure def sendResponse(buffer: Address -> Set[ResponseMsg], m: ResponseMsg): Address -> Set[ResponseMsg] =
    buffer.put(m.client, { ...buffer.get(m.client).union(Set(m)) })

  action initVsync(nodes) = all {
    statusBuffer' = nodes.mapBy(v => Set()),
    requestsBuffer' = nodes.mapBy(v => Set()),
    responsesBuffer' = nodes.mapBy (v => Set()),
  }

  action unchangedVsync = all {
    statusBuffer' = statusBuffer,
    requestsBuffer' = requestsBuffer,
    responsesBuffer' = responsesBuffer,
  }

}
