// -*- mode: Bluespec; -*-

// TODO: is the number of registrations in L2 block a limitting factor for the reset
// TODO: generate unsuccesful reset schenario
// TODO: Write properties in terms of valid, decided, invalidated, finalized blocks

// TODO: L1->L2 messaging uses nonce for at-most-once delivery / but delivery might be out of order

// TODO: replace nextValset by staged, unstaged Registration logic

// TODO: Time between block creation and proof on L1 must be big enough to also have proof on L2

// quint run --invariant "noReset" reset.qnt 


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
    | L2Proof((Height,Set[Registration],ChainIDs)) // aggregated heights TODO: add set of proods 
    | Nil

type L1Block = {
    // meta data
    height : int,
    // transactions
    newRegistrations: Set[Registration],
    proof: Proof, // Set[Proof],
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
    stagedUpdates: Set[Registration],
    unstagedUpdates: Set[Registration],
    // history of received registrations needed for proof (for analysis. history variable)
    // TODO: actually it is needed to ensure at-most once. I guess we will do 
    // "registrations \setminus receivedRegistrations
    receivedRegistrations: Set[Registration],
    // proofs not modelled here
}


pure def staleRegs (b: L1Block) : Set[Registration] =
    b.pendingRegistrations.filter(r => r.epoch + 1 < b.height / L1EpochSize)


pure def updatePending (new: Set[Registration], pending: Set[Registration], confirmed: Set[Registration]) : Set[Registration] =
    pending.union(new).exclude(confirmed)

pure def confirmedRegs (proof: Proof)  : Set[Registration] =
    match proof {
        | Nil => Set()
        | L2Proof(p) => p._2
    }

pure def allConfirmed (proof: Proof, prev: L1Block, epoch: int) : bool = {
    // TODO: figure out whether all pending registrations are confirmed
    
    true
}

// pure def correctForkProved(chain: List[L1Block], proofs: Set[Proof]) : bool = 
//    true // there exists a proof with: correct forkID, all pending registrations confirmed

// // This is how newL1Block should look like if proofs would become a set
// pure def newL1Block (chain: List[L1Block], regs: Set[Registration], proofs: Set[Proof]) : L1Block =
//     val prev = chain[chain.length() - 1]
//     val auxBlock = {    height : chain.length(),
//                         newRegistrations: regs,
//                         proof: prev.proof, //proofs,  todo: transfer to set
//                         pendingRegistrations: prev.pendingRegistrations.union(regs),
//                         provenHeight: 1, // TODO remove
//                         cntRegistrations: prev.cntRegistrations + regs.size()
//     }
//     // If there are no stale registrations or ther are stale registrations but the proofs contain a proof
//     //   for a good fork block, we add the proof and remove confirmed registrations from pending ones
//     // Else, we reject the proof, and thus don't have confirmations
//     val blockWithGoodProof = 
//         {... auxBlock, pendingRegistrations: auxBlock.pendingRegistrations.exclude(confirmedRegs(auxBlock.proof))}
//     val blockWithRejectedProof = 
//         {... auxBlock, proof: Nil}     
//     if (not(existsStale(prev))) // TODO: one-off question
//         blockWithGoodProof
//     else
//         if (correctForkProved(chain, proofs))
//             blockWithGoodProof
//         else 
//             blockWithRejectedProof

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
                // If there are stale registrations, I only accept a proof for a block whose 
                // chainID "corresponds to" current EVE epoch
                (pCID == currentEpoch and // L 1/ EVE. First block the proof represents contains valset with all 
                // PCID: should be forkID
                // should not be current epoch but epoch of stale registrations
                // all stale registrations are confirmed
                allConfirmed(newProof, prev, currentEpoch) and
                // and height sanity check:
                pHeight > prev.provenHeight))
                blockWithProof
            else 
                blockWithoutProof
    }

// pure def updatePendingUpdates (pending: Set[Registration], regs: Set[Registration]) : Set[Registration] =
//    pending.union(regs)

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
            valset: nextValSet(prev.valset, prev.unstagedUpdates),
            unstagedUpdates: regs,
            stagedUpdates: prev.unstagedUpdates,
            receivedRegistrations: prev.receivedRegistrations.union(regs),
        }   
    else
        {   height: chain.length(), 
            chainID: prev.chainID,
            registrations: regs, 
            valset: prev.valset, 
            stagedUpdates: prev.stagedUpdates, 
            unstagedUpdates: prev.unstagedUpdates.union(regs),
            receivedRegistrations: prev.receivedRegistrations.union(regs),
        }



pure def forkBlock (chain : List[L2Block], lastL1: L1Block) : L2Block =
    val prev = chain[chain.length() - 1]
    val regs = staleRegs(lastL1)
    val newValSet = nextValSet(prev.valset, prev.unstagedUpdates.union(prev.stagedUpdates).union(regs))
    // TODO: confirm if the intention above is OK?
    // TODO: there is a bug in registrations are doubly counted if already in nextValSet 
    //       (registrations are not "at most once" in this model)
    // unstagedUpdates could be filtered for epochs? 
    // Is L1->L2 messaging guaranteed to be ordered?
    {   height: lastL1.provenHeight + 1, 
        chainID: lastL1.height % L1EpochSize,
        registrations: regs, 
        valset: newValSet,  
        stagedUpdates: Set(),
        unstagedUpdates: Set(),
        receivedRegistrations: prev.receivedRegistrations.union(regs),
    }

pure def resetL2 (chain : List[L2Block], lastL1: L1Block) : List[L2Block] = 
    chain.slice(0, lastL1.provenHeight + 1).append(forkBlock(chain, lastL1))


// STATE MACHINE

var L1: List[L1Block]
var L2: List[L2Block]
var envRegs: Set[Registration]

// Invariants
pure def existsStale (b: L1Block) : bool =
    b.pendingRegistrations.exists(r => r.epoch + 1 < b.height / L1EpochSize)
    
def staleInv = not(L1[L1.length() - 1].existsStale())


// chainID of last element is 0
// if used as invariant and violated, we have an example with a reset
def noReset = L2[L2.length() - 1].chainID == 0


// ACTIONS

action init = all {
    L1' = List({height : 0,
                newRegistrations: Set(),
                proof: Nil, // Set(Nil),
                pendingRegistrations: Set(),
                provenHeight: -1,
                cntRegistrations: 0,
                }),
    L2' = List({height: 0,
                chainID: 0,
                registrations: Set(),
                valset: Map(),
                stagedUpdates: Set(),
                unstagedUpdates: Set(),
                receivedRegistrations: Set(),
                }),
    envRegs' = Set(),
}

action addRegistration = all {
    nondet newVal = ValAddresses.oneOf()
    nondet power = minReg.to(maxReg).oneOf()
    val newReg = {
        epoch: L1.length() / L1EpochSize,
        index: L1[L1.length() - 1].cntRegistrations + envRegs.size()  + 1, // encodes unique registration index
        address: newVal,
        update: power
    }
    envRegs' = envRegs.union(Set(newReg)),
    L1' = L1,
    L2' = L2,
}

action addL1Block = all {
    nondet proofHeight = (-1).to(L2.length() - 1).oneOf()
    val proof = if (proofHeight == -1) Nil
                else L2Proof((proofHeight, L2[proofHeight].receivedRegistrations, L2[proofHeight].chainID)) 
    L1' = L1.append(newL1Block(L1, envRegs, proof)),
    L2' = L2,
    envRegs' = Set(),
}

action addL2Block = all {
    nondet regs = L1[L1.length() - 1].pendingRegistrations.powerset().oneOf() 
    // TODO: Question: at most once?
    L2' = L2.append(newL2Block(L2, regs)),
    L1' = L1,
    envRegs' = envRegs,
}

action reset = all {
    val lastL1 = L1[L1.length() - 1]
    if (existsStale(lastL1))
        L2' = L2.resetL2(lastL1)
    else
        L2' = L2,
    L1' =  L1,
    envRegs' = envRegs,
}


action step = any {
    addRegistration,    // adds a registration to the environment. A bit like submitting a transaction
    addL1Block,         // adds a new L1 block, and takes all registrations from the environment
    addL2Block,         // adds a normal L2 block / no reset
    reset               // this is asynchronous now. We should link it to L1 block generation
}


run simpleTest = {
    all {
    init
    .then(all{assert(L1.length() == 1), addL1Block})
    .then(all{assert(L1.length() == 2), addL1Block})
    }
}

}