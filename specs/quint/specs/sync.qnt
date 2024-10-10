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

type SynchronizerOutput = 
    | SOCertificate(Set[Vote])
    | SOBlock(Proposal)
    | SONoOutput

/// The state of the synchronizer
type Synchronizer = {
    id: Address,
    height: Height,
    peers: Set[Address],    // MVP: this is going to be the validator set for now
                            // so we use addresses. We might use peerID in the future
    peerStatus: Address -> SyncStatus,
    openRequests: Set[RequestMsg],
    lastSyncedHeight: Height,   // "done" if greater than or equal to height
                                // TODO: we could add buffers for certificates and values
    // inbuffers
    statusMsgs: Set[SyncStatusMsg],
    responses: Set[ResponseMsg],
}

pure def initSynchronizer(id: Address, peers: Set[Address]) : Synchronizer =
    {   id: id,
        height: -1,
        peers: peers,
        peerStatus: peers.mapBy(x => {base:-1, top:-1}),
        openRequests: Set(),
        lastSyncedHeight: -1,
        statusMsgs: Set(),
        responses: Set(),
    }

/// The server state are just incoming requests. The actual data used to respond
/// is from the NodeState
type Server = {
    inbuffer : Set[RequestMsg]
}

pure def initServer = {inbuffer : Set()}

/// auxiliary function to iterate over the received status messages
pure def updatePeerStatus (ps: Address -> SyncStatus, msgs: Set[SyncStatusMsg]) : Address -> SyncStatus =
    msgs.fold(ps, (newStatus , msg) => 
        if (newStatus.get(msg.peer).top < msg.top) // TODO: think about base?
            newStatus.put(msg.peer, {base: msg.base, top: msg.top})
        else
            newStatus
    )

/// inform the synchronizer that consensus has entered height h
pure def syncNewHeight (s: Synchronizer, h: Height) : Synchronizer =
    if (h <= s.height) s
    else
        s.with("height", h)

/// look into the node state and generate a status message
pure def syncStatus (s: NodeState) : SyncStatusMsg =
// TODO: perhaps we should add to height to the chain entries to capture non-zero bases
    { peer: s.es.cs.p , base: 0, top: s.es.chain.length() - 1  }




pure def broadcastStatus(buffer: Address -> Set[SyncStatusMsg], sm: SyncStatusMsg): Address -> Set[SyncStatusMsg] =
    buffer.keys().mapBy(x => { ...buffer.get(x).union(Set(sm)) })

/// put the response message in the boffer of the client
pure def sendResponse(buffer: Address -> Set[ResponseMsg], m: ResponseMsg): Address -> Set[ResponseMsg] =
    buffer.put(m.client, { ...buffer.get(m.client).union(Set(m)) })


//
// State machine
//

var syncSystem: Address -> Synchronizer
var serverSystem: Address -> Server

var statusBuffer : Address -> Set[SyncStatusMsg]  
var syncResponseBuffer : Address -> Set[ResponseMsg]
var syncRequestBuffer : Address -> Set[RequestMsg]

action syncInit = all {
    statusBuffer' = Correct.mapBy(v => Set()),
    syncResponseBuffer' = Correct.mapBy(v => Set()),
    syncRequestBuffer' = Correct.mapBy(v => Set()),
    syncSystem' = Correct.mapBy(v => initSynchronizer(v, validators)),
    serverSystem' = Correct.mapBy(v => initServer),    
}

/// environment sends the node to the next height.
/// checks that we have all the previous blocks in the store.
action newHeightAction(v, valset, h) = all {
    system.get(v).es.chain.length() >= system.get(v).es.cs.height,
    val ns = system.get(v).with("es", initDriver(v, valset, h, system.get(v).es.chain))
    system' = system.put(v, ns), 
    syncSystem' = syncSystem.put(v, syncNewHeight(syncSystem.get(v), h)),
    // rest unchanged
    statusBuffer' = statusBuffer,
    syncResponseBuffer' = syncResponseBuffer,
    serverSystem' = serverSystem,
    syncRequestBuffer' = syncRequestBuffer,
    _hist' = _hist,
    propBuffer' = propBuffer,
    voteBuffer' = voteBuffer,
    certBuffer' = certBuffer,
}

/// initialize consensus and synchronizer
action initAll = all {
    init,
    syncInit
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
        val goodPeers = s.peers.filter(p => newPeerStates.get(p).base <= s.height   
                                            and s.height <= newPeerStates.get(p).top )
        if (goodPeers.size() > 0)
            if (s.openRequests.size() == 0) 
                // we start the sync "round" by asking for a certificate
                val req = { client: s.id,
                            server: goodPeers.fold("", (acc, i) => i), //chooseSome(),
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
                    val resp = s.responses.fold(emptyResponseMsg, (acc, i) => i) //chooseSome() // in the future there might be parallelization
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
                                                server: goodPeers.fold("", (acc, i) => i), // chooseSome(),
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
        val m = s.inbuffer.fold(emptyReqMsg, (acc, i) => i) // chooseSome() // TODO: fix
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
        workaround(v, result),
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

pure def setChain(s: DriverState, c: List[{decision: Proposal, commit: Set[Vote]}]): DriverState =
    {... s, chain: c}




pure def commitSet (h: Height, v: Value) : Set[Vote] =
    Set("v1", "v2", "v3").map(n => mkVote(Precommit, n, h, 0, v))


/// this sets up an initial state where v4 starts late, and v2 and v3 have reached
/// height h
action initHeight(h) = all {
    val special = "v4" // TODO proper selection from correct set
    val initsystem = Correct.mapBy(v => 
        // hack
            if (v == special) initNode(v, validatorSet, 0)
            else initNode(v, validatorSet, h))
    nondet decisionValMap = 0.to(h-1).setOfMaps(Values).oneOf()
    val propMap =  decisionValMap.keys().mapBy(i => 
                    mkProposal( proposer(validatorSet, i,0), 
                                i, 
                                0, 
                                decisionValMap.get(i), 
                                0))
    val list = range(0, h).foldl(List(), (acc, i) => acc.append(propMap.get(i)))
    val chain = list.foldl(List(), (acc, i) =>  acc.append({decision: i, commit: commitSet(i.height, Val(i.proposal))}))
    all {
        system' = initsystem.keys().mapBy(x => 
        // hack
            if (x == special) initsystem.get(x)
            else {... initsystem.get(x), es: setChain(initsystem.get(x).es, chain) }),
    propBuffer' = Correct.mapBy(v => Set()),
    voteBuffer' = Correct.mapBy(v => Set()),
    certBuffer' = Correct.mapBy(v => Set()),
    _hist' = { validator: "INIT", input: NoDInput, output: NoConsensusOutput },
    syncInit
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

action syncDeliverReq(v) =  all {
    syncRequestBuffer.get(v).size() > 0,
    nondet req = syncRequestBuffer.get(v).oneOf()
    all {
    syncRequestBuffer' = syncRequestBuffer.put(v, syncRequestBuffer.get(v).exclude(Set(req))),
    serverSystem' = serverSystem.put(v, {... serverSystem.get(v), 
                                inbuffer: serverSystem.get(v).inbuffer.union(Set(req))}),
    statusBuffer' = statusBuffer,
    syncResponseBuffer' = syncResponseBuffer,
    syncSystem' = syncSystem,
    unchangedAll
    }
}

action syncDeliverResp(v) =  all {
    syncResponseBuffer.get(v).size() > 0,
    nondet resp = syncResponseBuffer.get(v).oneOf()
    all {
        syncSystem' = syncSystem.put(v, {... syncSystem.get(v), 
                                responses: syncSystem.get(v).responses.union(Set(resp))}),
        syncRequestBuffer' = syncRequestBuffer,
        serverSystem' = serverSystem,
        statusBuffer' = statusBuffer,
        syncResponseBuffer' = syncResponseBuffer.put(v, syncResponseBuffer.get(v).exclude(Set(resp))),
        unchangedAll
    }
}

action syncDeliverStatus(v) =  all {
    statusBuffer.get(v).size() > 0,
    nondet status = statusBuffer.get(v).oneOf()
    all {
        syncSystem' = syncSystem.put(v, {... syncSystem.get(v), 
                                statusMsgs: syncSystem.get(v).statusMsgs.union(Set(status))}),
        syncRequestBuffer' = syncRequestBuffer,
        serverSystem' = serverSystem,
        statusBuffer' = statusBuffer.put(v, statusBuffer.get(v).exclude(Set(status))),
        syncResponseBuffer' = syncResponseBuffer,
        unchangedAll
    }
}

action syncValStep(v) = all {
    valStep(v),
    syncUnchangedAll
}

action stepWithBlockSync = any {
    all {
        step,
        syncUnchangedAll
    },
    nondet v = oneOf(Correct)
    any {
        syncDeliverReq(v),
        syncDeliverResp(v),
        syncDeliverStatus(v),
        syncStepServer(v),   //looking for a request and sends responses
        syncStepClient(v), //   is checking if there are responses and check whether it need to requ
        syncStatusStep(v),
        //syncTimeout(v) // TODO:
    }
}

run syncCycle(h) = 
    newHeightAction("v4", validatorSet, h)
    .then(syncStatusStep("v2"))
    .then(syncDeliverStatus("v4"))
    .then(syncStepClient("v4")) // ask for certificate
    .then(syncDeliverReq("v2"))
    .then(syncStepServer("v2"))
    .then(syncDeliverResp("v4"))
    .then(syncStepClient("v4")) // ask for block and give certificate to node
    .expect(system.get("v4").incomingSyncCertificates.size() > h)
    .then(syncDeliverReq("v2"))
    .then(syncStepServer("v2"))
    .then(syncDeliverResp("v4"))
    .then(syncStepClient("v4"))    
    .expect(system.get("v4").incomingSyncProposals.size() > h)
    .then(3.reps(_ => syncValStep("v4")))
    .expect(system.get("v4").es.chain.length() > 0)

run retreat = 
    initHeight(4)
    .then(syncCycle(0)) // TODO figure out iteration
    // and now v4 has synced block 1!

    


}