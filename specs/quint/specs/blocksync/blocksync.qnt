// -*- mode: Bluespec; -*-

module blocksync {

	import types.* from "../types"
	export types.*

	type Option[a] =
	    | Some(a)
	    | None

	type StatusMsg = {
	    peer: Address,
	    base: Height,
	    top: Height
	}

	type BlockRange = {
	    base: Height,
	    top: Height
	}

	type ReqType =
	    | SyncCertificate
	    | SyncBlock

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
	    | RespBlock(Proposal)
	    | RespCertificate(Set[Vote])

	type ResponseMsg = {
	    client: Address,
	    server: Address,
	    height: Height,
	    response: Response,
	}

	pure def emptyResponseMsg = {
	    client: "",
	    server: "",
	    height: -1,
	    response: RespBlock(emptyProposal),
	}

	// *************
	// State machine
	// *************

	// Messages exchanged by nodes (clients and servers)
	var statusBuffer : Address -> Set[StatusMsg]
	var requestsBuffer : Address -> Set[RequestMsg]
	var responsesBuffer : Address -> Set[ResponseMsg]

	// Auxiliary functions for sending messages

	pure def broadcastStatusMsg(buffer: Address -> Set[StatusMsg], sm: StatusMsg): Address -> Set[StatusMsg] =
	    buffer.keys().mapBy(x => { ...buffer.get(x).union(Set(sm)) })

	// put the response message in the buffer of the client
	pure def sendResponse(buffer: Address -> Set[ResponseMsg], m: ResponseMsg): Address -> Set[ResponseMsg] =
	    buffer.put(m.client, { ...buffer.get(m.client).union(Set(m)) })

	action initBsync(nodes) = all {
	    statusBuffer' = nodes.mapBy(v => Set()),
	    requestsBuffer' = nodes.mapBy(v => Set()),
	    responsesBuffer' = nodes.mapBy (v => Set()),
	}

	action unchangedBsync = all {
	    statusBuffer' = statusBuffer,
	    requestsBuffer' = requestsBuffer,
	    responsesBuffer' = responsesBuffer,
	}
}
