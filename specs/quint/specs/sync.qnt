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



// ****************************************************************************
// Synchronizer (client side)
// ****************************************************************************


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

//
// Synchronizer functions
//

/// Initialize the synchronizer
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

/// Auxiliary function to iterate over the received status messages
pure def updatePeerStatus (ps: Address -> SyncStatus, msgs: Set[SyncStatusMsg]) : Address -> SyncStatus =
    msgs.fold(ps, (newStatus , msg) => 
        if (newStatus.get(msg.peer).top < msg.top) // TODO: think about base?
            newStatus.put(msg.peer, {base: msg.base, top: msg.top})
        else
            newStatus
    )

/// inform the synchronizer that consensus has entered height h
pure def syncNewHeight (s: Synchronizer, h: Height) : Synchronizer =
    if (h <= s.height) 
        s
    else
        s.with("height", h)

/// returned by the synchronizer: sync is the new state, so is the output towards
/// the consensus driver, req are messages sent towards peers/servers
type ClientResult = {
    sync: Synchronizer, 
    so: SynchronizerOutput, 
    req: Option[RequestMsg]
}

/// We have received a certificate. now we need to issue the corresponding block request
/// and generate a certificate output
pure def syncHandleCertificate (s: Synchronizer, cert: Set[Vote], peer: str ) : ClientResult =
    val blockReq = {    client: s.id,
                        server: peer, 
                        rtype: SyncBlock,
                        height: s.height}
    {   sync: {...s, openRequests: Set(blockReq)}, // If we have parallelization we need to be more precise here
        so: SOCertificate(cert),
        req: Some(blockReq)}

/// we have received a block. now we need to  generate a block output
pure def syncHandleBlock (s: Synchronizer, p: Proposal) : ClientResult =
    {   sync: {...s,    openRequests: Set(), // If we have parallelization we need to remove one element,
                        lastSyncedHeight: s.height }, // blockheight,
        so: SOBlock(p),
        req: None}

/// step of a client:
/// 1. update peer statuses, 2. if there is no open request, request something
/// 3. otherwise check whether we have a response and act accordingly
pure def syncClient (s: Synchronizer) : ClientResult =
    val newPeerStates = updatePeerStatus(s.peerStatus, s.statusMsgs)
    val newS = { ...s, peerStatus: newPeerStates}
    if (s.lastSyncedHeight >= s.height)
        // nothing to do
        {   sync: newS,
            so: SONoOutput,
            req: None}
    else
        val goodPeers = s.peers.filter(p => newPeerStates.get(p).base <= s.height   
                                            and s.height <= newPeerStates.get(p).top )
        if (goodPeers.size() > 0)
            if (s.openRequests.size() == 0) 
                // we start the sync "round" by asking for a certificate
                val req = { client: s.id,
                            server: goodPeers.fold("", (acc, i) => i), //chooseSome(),
                            rtype: SyncCertificate,
                            height: s.height}
                {   sync: {... newS, openRequests: s.openRequests.union(Set(req))},
                    so: SONoOutput,
                    req: Some(req)
                }
            else
                // we issued a request before, let's see whether there is a response
                if (s.responses.size()> 0)
                    val resp = s.responses.fold(emptyResponseMsg, (acc, i) => i) //chooseSome() // in the future there might be parallelization
                    
                    match resp.response {
                        | RespBlock(prop) => syncHandleBlock(newS, prop)
                        | RespCertificate(cert) => syncHandleCertificate(newS, cert, goodPeers.fold("", (s,x) => x))
                    }
                else 
                    // I don't have response 
                    // this might be timeout logic
                    {   sync: newS,
                        so: SONoOutput,
                        req: None}
        else
            // no peers
            {   sync: newS,
                so: SONoOutput,
                req: None}




// ****************************************************************************
// Server
// ****************************************************************************


/// The server state are just incoming requests. The actual data used to respond
/// is from the NodeState
type Server = {
    inbuffer : Set[RequestMsg]
}

//
// Server functions
//

pure def initServer = {inbuffer : Set()}


/// look into the node state and generate a status message
pure def syncStatus (s: NodeState) : SyncStatusMsg =
// TODO: perhaps we should add to height to the chain entries to capture non-zero bases
    { peer: s.es.cs.p , base: 0, top: s.es.chain.length() - 1  }

/// new server state and response messages to be sent
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








// ****************************************************************************
// State machine
// ****************************************************************************




//
// The statemachine is put on top of statemachineAsync, that is, we use its
// initialization and steps, and add the updates to the variables defined below
//



// Variables

var syncSystem: Address -> Synchronizer
var serverSystem: Address -> Server

var statusBuffer : Address -> Set[SyncStatusMsg]  
var syncResponseBuffer : Address -> Set[ResponseMsg]
var syncRequestBuffer : Address -> Set[RequestMsg]


// Auxiliary functions for sending messages

pure def broadcastStatus(buffer: Address -> Set[SyncStatusMsg], sm: SyncStatusMsg): Address -> Set[SyncStatusMsg] =
    buffer.keys().mapBy(x => { ...buffer.get(x).union(Set(sm)) })

/// put the response message in the buffer of the client
pure def sendResponse(buffer: Address -> Set[ResponseMsg], m: ResponseMsg): Address -> Set[ResponseMsg] =
    buffer.put(m.client, { ...buffer.get(m.client).union(Set(m)) })

//
// Actions
//

/// initializing the variables of the sync part of the state machine 
action syncInit = all {
    syncSystem' = Correct.mapBy(v => initSynchronizer(v, validators)),
    serverSystem' = Correct.mapBy(v => initServer),    
    statusBuffer' = Correct.mapBy(v => Set()),
    syncResponseBuffer' = Correct.mapBy (v => Set()),
    syncRequestBuffer' = Correct.mapBy(v => Set()),
}

action syncUnchangedAll = all {
    serverSystem' = serverSystem,
    syncRequestBuffer' = syncRequestBuffer,
    statusBuffer' = statusBuffer,
    syncResponseBuffer' = syncResponseBuffer,
    syncSystem' = syncSystem,
}

/// initialize consensus and synchronizer
action initAll = all {
    init,
    syncInit
}

//
// Actions for the Environment to send a node to a new height
//

/// environment sends the node to the next height.
/// initializes synchronizer
action newHeightActionSync(v, valset, h) = all {
    syncSystem' = syncSystem.put(v, syncNewHeight(syncSystem.get(v), h)),
    statusBuffer' = statusBuffer,
    syncResponseBuffer' = syncResponseBuffer,
    serverSystem' = serverSystem,
    syncRequestBuffer' = syncRequestBuffer,
}

/// environment sends the node to the next height.
action newHeightActionAll(v, valset, h) = all {
    newHeightActionSync(v, valset, h),
    newHeightAction(v, valset, h),
}

//
// Actions for process steps in the sync protocol
//

/// Server v announces its status
action syncStatusStep(v) = all {
    val newStatus = system.get(v).syncStatus()
    statusBuffer' = broadcastStatus(statusBuffer, newStatus), 
    syncResponseBuffer' = syncResponseBuffer,
    syncSystem' = syncSystem,
    serverSystem' = serverSystem,
    syncRequestBuffer' = syncRequestBuffer,
    unchangedAll
}

/// Server v takes a step (checking for requests and responding)
action syncStepServer(v) = all {
    val result = syncServer(serverSystem.get(v), system.get(v)) // this is where the chains is passed
    all {
        serverSystem' = serverSystem.put(v, result.sync),
        syncResponseBuffer' = match result.msg {
            | Some(m) => syncResponseBuffer.sendResponse(m) // TODO: fix after broadcast
            | None => syncResponseBuffer
        },
        syncSystem' = syncSystem,
        syncRequestBuffer' = syncRequestBuffer,
        statusBuffer' = statusBuffer, 
        unchangedAll,
    }
}

/// Client v takes a step
action syncStepClient(v) = all {
    val result = syncClient(syncSystem.get(v))
    all {
        syncSystem' = syncSystem.put(v, result.sync),
        syncRequestBuffer' = match result.req {
            | Some(m) => syncRequestBuffer.put(m.server, syncRequestBuffer.get(m.server).union(Set(m)))
            | None => syncRequestBuffer
        },        
        putSyncOutputIntoNode(v, result.so),    // the logic's code should be here, but due to Quint issues
                                                // we need to keep it in stateMachineAsync for now
        statusBuffer' = statusBuffer, 
        syncResponseBuffer' = syncResponseBuffer,
        serverSystem' = serverSystem,
    }
}




//
// Actions for the environment to deliver messages in the sync protocol
//

/// deliver a request to server v
/// remove the request from the syncRequestBuffer and add it to the server's inbuffer.
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

/// deliver a response to client v
/// remove the response from the syncResponseBuffer and add it to the client's responses buffer.
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

/// deliver a status message to client v
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

/// validator step in the system with sync protocol
action syncValStep(v) = all {
    valStep(v),
    syncUnchangedAll
}

/// main step function: either a consensus state-machine step or a sync protocol step
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



//
// Interesting scenarios
//

/// auxiliary function for initHeight
/// sets the chain
pure def setChain(s: DriverState, c: List[{decision: Proposal, commit: Set[Vote]}]): DriverState =
    {... s, chain: c}

/// auxiliary function for initHeight
/// constructs a commit certificate for a height and value
pure def commitSet (h: Height, v: Value) : Set[Vote] =
    Set("v1", "v2", "v3").map(n => mkVote(Precommit, n, h, 0, v))

/// An action to set up an initial state with some nodes already decided up to height h
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

/// a simple scenario where v4 starts height h
run syncCycle(h) = 
    newHeightActionAll("v4", validatorSet, h)
    .then(syncStatusStep("v2"))
    .then(syncDeliverStatus("v4"))
    .then(syncStepClient("v4")) // ask for certificate
    .then(syncDeliverReq("v2"))
    .then(syncStepServer("v2"))
    .then(syncDeliverResp("v4"))
    .then(syncStepClient("v4")) // ask for block and give certificate to node
    .expect(system.get("v4").incomingSyncCertificates.size() > 0)
    .then(syncDeliverReq("v2"))
    .then(syncStepServer("v2"))
    .then(syncDeliverResp("v4"))
    .then(syncStepClient("v4"))    
    .expect(system.get("v4").incomingSyncProposals.size() > 0)
    .then(3.reps(_ => syncValStep("v4")))
    .expect(system.get("v4").es.chain.length() > h)

run retreat = 
    nondet heightToReach = 1.to(4).oneOf()
    initHeight( q::debug ("Height:", heightToReach))
    .then(heightToReach.reps(i => syncCycle(i)))
    .expect(system.get("v4").es.chain == system.get("v2").es.chain)
    .then(newHeightActionAll("v4", validatorSet, heightToReach))
    .expect(system.get("v4").es.cs.height == system.get("v2").es.cs.height)
    // and now v4 has synced !

    


}