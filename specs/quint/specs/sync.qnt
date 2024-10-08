// -*- mode: Bluespec; -*-

module sync {
    
import statemachineAsync(
    validators = Set("v1", "v2", "v3", "v4"),
    validatorSet = Set("v1", "v2", "v3", "v4").mapBy(x => 1),
    Faulty = Set("v1"),
    Values = Set("red", "blue"),
    Rounds = Set(0, 1, 2, 3),
    Heights = Set(0) // , 1, 2, 3)
    ).* from "./statemachineAsync"

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

type SynchronizerOutput = 
    | SOCertificate(Set[Vote])
    | SOBlock(Value)
    | SONoOutput

type Option[a] =
    | Some(a)
    | None

type Synchronizer = {
    height: Height,
    peers: Set[Address],    // MVP: this is going to be the validator set for now
                            // so we use addresses. We might use peerID in the future
    peerStatus: Address -> SyncStatus,
    openRequests: Set[RequestMsg],
    lastSyncedHeight: Height // done if greater than or equal to height
}

pure def initSynchronizer(peers: Set[Address]) : Synchronizer =
    {
        height: -1,
        peers: peers,
        peerStatus: peers.mapBy(x => {base:-1, top:-1}),
        openRequests: Set(),
        lastSyncedHeight: -1,
    }

pure def syncNewHeight (s: Synchronizer, h: Height) : Synchronizer =
    if (h <= s.height) s
    else
        s.with("height", h)


pure def syncStatus (s: NodeState) : SyncStatusMsg =
//TODO: Logic
    { peer: s.es.cs.p , base: 5, top: 7 }



// State machine

var statusBuffer : Address -> Set[SyncStatusMsg]  
var syncSystem: Address -> Synchronizer


pure def sendStatus(buffer: Address -> Set[SyncStatusMsg], sm: SyncStatusMsg): Address -> Set[SyncStatusMsg] =
    buffer.keys().mapBy(x => { ...buffer.get(x).union(Set(sm)) })


 

action syncInit = all {
    init,
    statusBuffer' = Correct.mapBy(v => Set()),
}

action syncUnchangedAll = all {
    statusBuffer' = statusBuffer
}


pure def syncClient (s: Synchronizer) :
{sync: Synchronizer, so: SynchronizerOutput, req: Option[RequestMsg]} =
// TODO logic
    {
        sync: s,
        so: SONoOutput,
        req: None
    }

pure def syncServer (s: Synchronizer, ns: NodeState) :
{sync: Synchronizer, block: Value, cert: Set[Set[Vote]]} =
// TODO logic
// check for requests and serve them
// coupling to node state unclear
    {
        sync: s,
        block: Val("1"),
        cert: Set()
    }

action syncStepServer(v) = all {
   unchangedAll,
    statusBuffer' = statusBuffer 

}

action syncStepClient(v) = all {
    // TODO call syncClient and deliver the SO to the node 
    unchangedAll,
    statusBuffer' = statusBuffer 
}


action syncStatusStep(v) = all {
    val newStatus = system.get(v).syncStatus()
    statusBuffer' = sendStatus(statusBuffer, newStatus),
    unchangedAll
}


action stepWithBlockSync = any {
    all {
        step,
        syncUnchangedAll
    },
    nondet v = oneOf(Correct)
    any {
        //syncDeliverReq(v),
        //syncDeliverResp(v),
        syncStepServer(v),   //looking for a request and sends responses
        syncStepClient(v), //   is checking if there are responses and check whether it need to requ
        syncStatusStep(v),
        //syncTimeout(v)
    }
}

}
