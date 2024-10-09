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


type Response = 
    | RespBlock(NonNilValue)
    | RespCertificate(Set[Vote])

type ResponseMsg = {
    client: Address,
    server: Address,
    height: Height,
    response: Response,
}



type SynchronizerOutput = 
    | SOCertificate(Set[Vote])
    | SOBlock(NonNilValue)
    | SONoOutput

type Option[a] =
    | Some(a)
    | None

type Synchronizer = {
    id: Address,
    height: Height,
    peers: Set[Address],    // MVP: this is going to be the validator set for now
                            // so we use addresses. We might use peerID in the future
    peerStatus: Address -> SyncStatus,
    openRequests: Set[RequestMsg],
    lastSyncedHeight: Height, // done if greater than or equal to height
    // we could add buffers for certificates and values
    statusMsgs: Set[SyncStatusMsg],
    responses: Set[ResponseMsg],
}

type Server = {
    inbuffer : Set[RequestMsg]
}

pure def initSynchronizer(id: Address, peers: Set[Address]) : Synchronizer =
    {
        id: id,
        height: -1,
        peers: peers,
        peerStatus: peers.mapBy(x => {base:-1, top:-1}),
        openRequests: Set(),
        lastSyncedHeight: -1,
        statusMsgs: Set(),
        responses: Set(),
    }

pure def updatePeerStatus (ps: Address -> SyncStatus, msgs: Set[SyncStatusMsg]) : Address -> SyncStatus =
    msgs.fold(ps, (newStatus , msg) => 
        if (newStatus.get(msg.peer).top < msg.top) // TODO: think about base?
            newStatus.put(msg.peer, {base: msg.base, top: msg.top})
        else
            newStatus
    )

pure def initServer = {inbuffer : Set()}

pure def syncNewHeight (s: Synchronizer, h: Height) : Synchronizer =
    if (h <= s.height) s
    else
        s.with("height", h)


pure def syncStatus (s: NodeState) : SyncStatusMsg =
//TODO: Logic
// TODO: perhaps we should add to height to the chain entries to capture non-zero bases
    { peer: s.es.cs.p , base: 0, top: s.es.chain.length() - 1  }



// State machine

var syncSystem: Address -> Synchronizer
var serverSystem: Address -> Server

var statusBuffer : Address -> Set[SyncStatusMsg]  
var syncResponseBuffer : Address -> Set[ResponseMsg]
var syncRequestBuffer : Address -> Set[RequestMsg]

pure def broadcastStatus(buffer: Address -> Set[SyncStatusMsg], sm: SyncStatusMsg): Address -> Set[SyncStatusMsg] =
    buffer.keys().mapBy(x => { ...buffer.get(x).union(Set(sm)) })

/// put the response message in the boffer of the client
pure def sendResponse(buffer: Address -> Set[ResponseMsg], m: ResponseMsg): Address -> Set[ResponseMsg] =
    buffer.put(m.client, { ...buffer.get(m.client).union(Set(m)) })



action newHeightAction(v, valset, h) = all {
    val ns = system.get(v).with("es", initDriver(v, valset, h))
    system' = system.put(v, ns), 
    syncSystem' = syncSystem.put(v, syncNewHeight(syncSystem.get(v), h)),
    // rest unchanged
    statusBuffer' = statusBuffer,
    syncResponseBuffer' = syncResponseBuffer,
    _hist' = _hist,
    propBuffer' = propBuffer,
    voteBuffer' = voteBuffer,
    certBuffer' = certBuffer,
}

action syncInit = all {
    init,
    statusBuffer' = Correct.mapBy(v => Set()),
    syncResponseBuffer' = Correct.mapBy(v => Set()),
    syncRequestBuffer' = Correct.mapBy(v => Set()),
    syncSystem' = Correct.mapBy(v => initSynchronizer(v, validators)),
    serverSystem' = Correct.mapBy(v => initServer),    
}

action syncUnchangedAll = all {
    statusBuffer' = statusBuffer,
    syncResponseBuffer' = syncResponseBuffer,
    syncSystem' = syncSystem,
    serverSystem' = serverSystem,
    syncRequestBuffer' = syncRequestBuffer,
}


type ClientResult = {
    sync: Synchronizer, 
    so: SynchronizerOutput, 
    req: Option[RequestMsg]
}


// {
//     height: Height,
//     peers: Set[Address],    // MVP: this is going to be the validator set for now
//                             // so we use addresses. We might use peerID in the future
//     peerStatus: Address -> SyncStatus,
//     openRequests: Set[RequestMsg],
//     lastSyncedHeight: Height, // done if greater than or equal to height
//     // we could add buffers for certificates and values
//     statusMsgs: Set[SyncStatusMsg],
//     responses: Set[ResponseMsg],
// }

pure def syncClient (s: Synchronizer) : ClientResult =
    val newPeerStates = updatePeerStatus(s.peerStatus, s.statusMsgs)
    if (s.lastSyncedHeight >= s.height)
        // nothing to do
        {   sync: {...s, peerStatus: newPeerStates},
            so: SONoOutput,
            req: None}
    else
    // if there is a peer with a nice height, I am sending a request if I haven't done so before
    // if received response
        // if it is a certificate -> (check cerifcate) ask for the block from same peer
      //                              -> give the certificate to consensus
      //  if i receive a block -> (check block) give the block to consensus
        //                        ->  nothing (newheight comes from outside)
        val goodPeers = s.peers.filter(p => s.peerStatus.get(p).base <= s.height   
                                            and s.height < s.peerStatus.get(p).top )
        if (goodPeers.size() > 0)
            if (s.openRequests.size() == 0) 
                // we start the sync "round" by asking for a certificate
                val req = { client: s.id,
                            server: goodPeers.chooseSome(),
                            rtype: SyncCertificate,
                            height: s.height}
                {   sync: {...s,    
                                peerStatus: newPeerStates,
                                openRequests: s.openRequests.union(Set(req))},
                    so: SONoOutput,
                    req: Some(req)
                }
            else
                // we issued a request before, let's see whether there is a response
                if (s. responses.size()> 0)
                    val resp = s.responses.chooseSome() // in the future there might be parallelization
                    val output = match resp.response {
                        // to be given to the context // TODO: checks for heights, validity
                        | RespBlock(value) => SOBlock(value)
                        | RespCertificate(cert) => SOCertificate(cert)
                    }
                    // TODO: refactor into handleBlock and handleCertificate
                    val request = match resp.response {
                        // if we received a certificate, we need to ask for a block
                        | RespCertificate(cert) => 
                            val blockreq =  {   client: s.id,
                                                server: goodPeers.chooseSome(),
                                                rtype: SyncBlock,
                                                height: s.height}
                            Some(blockreq)
                        | RespBlock(value) => None
                    }
                    val newOpen = match resp.response {
                        // if we received a certificate, we need to record the block request, otherwise, we are done
                        | RespCertificate(cert) => 
                            match request {
                                | Some(blockreq) => Set(blockreq)
                                | None => Set()
                            }
                        | RespBlock(value) => Set() // If we have parallelization we need to remove one element
                    }
                    val newLast = match resp.response {
                        // if we received a certificate, we need to record the block request, otherwise, we are done
                        | RespCertificate(cert) => s.lastSyncedHeight
                        | RespBlock(value) => s.height // blockheight
                    }
                    {   sync: {...s, 
                                    peerStatus: newPeerStates,
                                    openRequests: newOpen,
                                    lastSyncedHeight: newLast},
                        so: output,
                        req: request}
                else 
                    // I don't have response 
                    // this might be timeout logic
                    {   sync: {...s, peerStatus: newPeerStates},
                        so: SONoOutput,
                        req: None}
        else
            // no peers
            {   sync: {...s, peerStatus: newPeerStates},
                so: SONoOutput,
                req: None}



type ServerOutput = {
    sync: Server, 
    msg: Option[ResponseMsg], 
}



pure def syncServer (s: Server, ns: NodeState) : ServerOutput =
    if (s.inbuffer.size() > 0) 
        val m = s.inbuffer.chooseSome() // TODO: fix
        val result =
            if (m.height < ns.es.chain.length())
                match m.rtype {
                    | SyncCertificate => 
                            val cm = {  client: m.client, 
                                        server: m.server, 
                                        height: m.height,
                                        response: RespCertificate(ns.es.chain[m.height].commit)}
                            Some(cm)
                    | SyncBlock  => 
                            val bl = {  client: m.client, 
                                        server: m.server, 
                                        height: m.height,
                                        response: RespBlock(ns.es.chain[m.height].decision)}
                            Some(bl)
                }
            else None
        {   sync: { ...s, inbuffer: s.inbuffer.exclude(Set(m))},
            msg: result}
    else {
        sync: s,
        msg: None}



action syncStepServer(v) = all {
    val result = syncServer(serverSystem.get(v), system.get(v)) // this is where the chains is passed
    all {
        unchangedAll,
        statusBuffer' = statusBuffer, 
        syncResponseBuffer' = match result.msg {
            | Some(m) => syncResponseBuffer.sendResponse(m) // TODO: fix after broadcast
            | None => syncResponseBuffer
        },
        syncSystem' = syncSystem,
        serverSystem' = serverSystem.put(v, result.sync),
        syncRequestBuffer' = syncRequestBuffer,
    }
}


action syncStepClient(v) = all {
    val result = syncClient(syncSystem.get(v))
    all {
        system' = match result.so {
            // Put the sync output into the consensus engine
            | SOCertificate(cert) => 
                system.put(v, {... system.get(v), 
                    incomingSyncCertificates:   
                    system.get(v).incomingCertificates.union(Set(cert))})
            | SOBlock(value) =>  
                system.put(v, 
                    {... system.get(v), 
                    incomingSyncValues: system.get(v).incomingSyncValues.union(Set(value))})
            | SONoOutput => system
        },
        propBuffer' = propBuffer,
        voteBuffer' = voteBuffer,
        certBuffer' = certBuffer,
        _hist' = _hist,
        statusBuffer' = statusBuffer, 
        syncResponseBuffer' = syncResponseBuffer,
        syncSystem' = syncSystem.put(v, result.sync),
        serverSystem' = serverSystem,
        syncRequestBuffer' = match result.req {
            | Some(m) => syncRequestBuffer.put(m.server, syncRequestBuffer.get(m.server).union(Set(m)))
            | None => syncRequestBuffer
        },
    }
}


action syncStatusStep(v) = all {
    val newStatus = system.get(v).syncStatus()
    statusBuffer' = broadcastStatus(statusBuffer, newStatus), 
    syncResponseBuffer' = syncResponseBuffer,
    syncSystem' = syncSystem,
    serverSystem' = serverSystem,
    syncRequestBuffer' = syncRequestBuffer,
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
       // syncStepClient(v), //   is checking if there are responses and check whether it need to requ
        syncStatusStep(v),
        //syncTimeout(v)
    }
}

}
