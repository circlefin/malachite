// -*- mode: Bluespec; -*-

module blocksync {

	import types.* from "../types"
	export types.*

	type Option[a] =
	    | Some(a)
	    | None

	type SyncStatusMsg = {
	    peer: Address,
	    base: Height,
	    top: Height
	}

	type SyncStatus = {
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

}
