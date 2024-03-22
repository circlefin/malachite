// -*- mode: Bluespec; -*-

module reset {

val L2EpochSize = 5
val L1EpochSize = 3
val maxReg = 3
val minReg = -3
val ValAddresses = Set("Huey", "Dewey", "Louie")

type Address = str
type Height = int
type ChainIDs = int

type Registration = {
    epoch: int,
    index: int,
    address: Address,
    update: int
}

type Proof = 
    | L2Proof((Height,Set[Registration],ChainIDs))
    | Nil

type L1Block = {
    // meta data
    height : int,
    // transactions
    newRegistrations: Set[Registration],
    proof: Proof,
    // state
    pendingRegistrations: Set[Registration],
    provenHeight: int,
    cntRegistrations: int,
}

type L2Block = {
    // meta data
    height: int,
    chainID: int,
    // transactions    
    registrations: Set[Registration],
    // state    
    valset: Address -> int,
    nextValset: Address -> int,
    pendingValsetUpdates: Set[Registration],
    // history of received registrations needed for proof
    receivedRegistrations: Set[Registration],
}

var L1: List[L1Block]
var L2: List[L2Block]
var envRegs: Set[Registration]

pure def existsStale (b: L1Block) : bool =
    b.pendingRegistrations.exists(r => r.epoch + 1 < b.height / L1EpochSize)
    
def staleInv = not(L1[L1.length() - 1].existsStale())

def noReset = L2[L2.length() - 1].chainID == 0

pure def staleRegs (b: L1Block) : Set[Registration] =
    b.pendingRegistrations.filter(r => r.epoch + 1 < b.height / L1EpochSize)

action init = all {
    L1' = List({height : 0,
                newRegistrations: Set(),
                proof: Nil,
                pendingRegistrations: Set(),
                provenHeight: -1,
                cntRegistrations: 0,
                }),
    L2' = List({height: 0,
                chainID: 0,
                registrations: Set(),
                valset: Map(),
                nextValset: Map(), // Question: initialized?
                pendingValsetUpdates: Set(),
                receivedRegistrations: Set(),
                }),
    envRegs' = Set(),
}

pure def updatePending (new: Set[Registration], pending: Set[Registration], confirmed: Set[Registration]) : Set[Registration] =
    pending.union(new).exclude(confirmed)

pure def confirmedRegs (proof: Proof)  : Set[Registration] =
    match proof {
        | Nil => Set()
        | L2Proof(p) => p._2
    }

pure def allConfirmed (proof: Proof, prev: L1Block, epoch: int) : bool = {
    // figure out whether all pending registrations 
    
    true
}

pure def newL1Block (chain: List[L1Block], regs: Set[Registration], proof: Proof) : L1Block =
    val prev = chain[chain.length() - 1]
    val newProof = match proof  {
                                | Nil => Nil
                                | L2Proof(p) => if (p._1 > prev.provenHeight) proof
                                                else Nil
    }
    val ph = match newProof {
                            | Nil => prev.provenHeight
                            | L2Proof(p) => p._1       
    }
    val blockWithProof = {  height : chain.length(),
                            newRegistrations: regs,
                            proof: newProof,
                            pendingRegistrations: updatePending(regs, prev.pendingRegistrations, confirmedRegs(proof)),
                            provenHeight: ph,
                            cntRegistrations: prev.cntRegistrations + regs.size() 
    } 
    val blockWithoutProof = {... blockWithProof, proof: Nil, provenHeight: prev.provenHeight}  
    match newProof {
        | Nil => blockWithProof
        | L2Proof(p) =>
            val pHeight = p._1
            val pRegs = p._2
            val pCID = p._3
            val currentEpoch = chain.length() / L1EpochSize
            if  (not(existsStale(prev)) // TODO: figure out which epochs to accept in the "normal" case without stale registrations
                or
                (pCID == currentEpoch and 
                allConfirmed(newProof, prev, currentEpoch) and
                pHeight > prev.provenHeight))
                blockWithProof
            else 
                blockWithoutProof
    }


action addL1Block = all {
    nondet proofHeight = (-1).to(L2.length() - 1).oneOf()
    val proof = if (proofHeight == -1) Nil
                else L2Proof((proofHeight, L2[proofHeight].receivedRegistrations, L2[proofHeight].chainID)) 
    L1' = L1.append(newL1Block(L1, envRegs, proof)),
    L2' = L2,
    envRegs' = Set(),
}

pure def updatePendingUpdates (pending: Set[Registration], regs: Set[Registration]) : Set[Registration] =
    pending.union(regs)

pure def addOneReg (vals: Address -> int, reg: Registration) : Address -> int =
    if (vals.keys().contains(reg.address))
        vals.set(reg.address, vals.get(reg.address) + reg.update)
    else
        vals.put(reg.address, reg.update)

pure def nextValSet (vals: Address -> int, regs: Set[Registration]) : Address -> int =
    regs.fold(vals, (v, r) => addOneReg(v,r))

pure def newL2Block (chain: List[L2Block], regs: Set[Registration]) : L2Block = 
    val prev = chain[chain.length() - 1]
    if (chain.length() % L2EpochSize == 0) // new epoch
        {   height: chain.length(), 
            chainID: prev.chainID,
            registrations: regs, 
            valset: prev.nextValset, 
            nextValset: nextValSet( prev.nextValset, 
                                    updatePendingUpdates(prev.pendingValsetUpdates, regs)),
            pendingValsetUpdates: Set(),
            receivedRegistrations: prev.receivedRegistrations.union(regs),
        }   
    else
        {   height: chain.length(), 
            chainID: prev.chainID,
            registrations: regs, 
            valset: prev.valset, 
            nextValset: prev.nextValset, 
            pendingValsetUpdates: updatePendingUpdates(prev.pendingValsetUpdates, regs),
            receivedRegistrations: prev.receivedRegistrations.union(regs),
        }

action addL2Block = all {
    nondet regs = L1[L1.length() - 1].pendingRegistrations.powerset().oneOf() // Question: at most once?
    L2' = L2.append(newL2Block(L2, regs)),
    L1' = L1,
    envRegs' = envRegs,
}

pure def forkBlock (chain : List[L2Block], lastL1: L1Block) : L2Block =
    val prev = chain[chain.length() - 1]
    val regs = staleRegs(lastL1)
    val newValSet = nextValSet(prev.nextValset, updatePendingUpdates(prev.pendingValsetUpdates, regs))
    {   height: lastL1.provenHeight + 1, 
        chainID: lastL1.height,
        registrations: regs, 
        valset: newValSet,  
        nextValset: newValSet, 
        pendingValsetUpdates: Set(),
        receivedRegistrations: prev.receivedRegistrations.union(regs),
    }

pure def resetL2 (chain : List[L2Block], lastL1: L1Block) : List[L2Block] = 
    chain.slice(0, lastL1.provenHeight + 1).append(forkBlock(chain, lastL1))

action reset = all {
    val lastL1 = L1[L1.length() - 1]
    if (existsStale(lastL1))
        L2' = L2.resetL2(lastL1)
    else
        L2' = L2,
    L1' = L1,
    envRegs' = envRegs,
}

action addRegistration = all {
    nondet newVal = ValAddresses.oneOf()
    nondet power = minReg.to(maxReg).oneOf()
    val newReg = {
        epoch: L1.length() / L1EpochSize,
        index: L1[L1.length() - 1].cntRegistrations + envRegs.size()  + 1,
        address: newVal,
        update: power
    }
    envRegs' = envRegs.union(Set(newReg)),
    L1' = L1,
    L2' = L2,
}

action step = any {
    addRegistration,    // adds a registration to the environment. A bit like submitting a transaction
    addL1Block,         // adds a new L1 block, and takes all registrations from the environment
    addL2Block,         // adds a normal L2 block / no reset
    reset               // this is asynchronous now. We should link it to L1 block generation
}

}