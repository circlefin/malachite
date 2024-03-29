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
type ChainID = int

type Registration = {
    epoch: int,
    index: int,
    address: Address,
    update: int
}

type L2BlockProof = {
    height: Height,
    confirmedRegs: Set[Registration],
    forkID: ChainID,
}

type L1Block = {
    // meta data
    height : Height,
    // transactions
    newRegistrations: Set[Registration],
    newProof: Set[L2BlockProof],
    // state
    pendingRegistrations: Set[Registration],
    verifiedProofs: Set[L2BlockProof],
    // provenHeight: int,
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

pure def L1Epoch(b: L1Block) : int =
    b.height % L1EpochSize

pure def staleRegs (b: L1Block) : Set[Registration] =
    b.pendingRegistrations.filter(r => r.epoch + 1 < b.height / L1EpochSize)

pure def existsStale (b: L1Block) : bool =
    staleRegs(b).size() > 0

pure def updatePending (new: Set[Registration], pending: Set[Registration], confirmed: Set[Registration]) : Set[Registration] =
    pending.union(new).exclude(confirmed)


pure def confirmedRegistrations (ps: Set[L2BlockProof]) : Set[Registration] = 
    ps.fold(Set(), (s, p) => s.union(p.confirmedRegs))


pure def provenHeight(b: L1Block) : Height =
    //TODO: smallest unproven block - 1
    val HeightsWithProofs = b.verifiedProofs.fold(Set(), (s, pp) => s.union(Set(pp.height)))
    val complete = HeightsWithProofs.filter(i => 0.to(i).subseteq(HeightsWithProofs))
    complete.fold(0, (max, i) => if (i > max) i else max)


// If no registrations are stale, I accept everything from past epochs
// otherwise, for heights greater they need to be from the current epoch
pure def proofOK (prev: L1Block, p: L2BlockProof) : bool =
    if (existsStale(prev))
        if (p.height < prev.provenHeight())
            p.forkID <= prev.L1Epoch() // TODO: not very precise
        else
            p.forkID == prev.L1Epoch() 
    else
        p.forkID <= prev.L1Epoch() // TODO: not very precise

pure def newL1Block (prev: L1Block, regs: Set[Registration], proofs: Set[L2BlockProof]) : L1Block =
    val accepted =  if (proofs.forall(p => proofOK(prev, p))) proofs
                    else Set()
    val confirmed = confirmedRegistrations(accepted)
    {   height : prev.height + 1,
        newRegistrations: regs,
        newProof: proofs,
        verifiedProofs: prev.verifiedProofs.union(accepted), 
        pendingRegistrations: prev.pendingRegistrations.union(regs).exclude(confirmed),
        cntRegistrations: prev.cntRegistrations + regs.size()
    }




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
    {   height: lastL1.provenHeight() + 1, 
        chainID: lastL1.L1Epoch(),
        registrations: regs, 
        valset: newValSet,  
        stagedUpdates: Set(),
        unstagedUpdates: Set(),
        receivedRegistrations: prev.receivedRegistrations.union(regs),
    }

pure def resetL2 (chain : List[L2Block], lastL1: L1Block) : List[L2Block] = 
    chain.slice(0, lastL1.provenHeight() + 1).append(forkBlock(chain, lastL1))


// STATE MACHINE

var L1: List[L1Block]
var L2: List[L2Block]
var envRegs: Set[Registration]

// Invariants
    
def staleInv = not(L1[L1.length() - 1].existsStale())


// chainID of last element is 0
// if used as invariant and violated, we have an example with a reset
def noReset = L2[L2.length() - 1].chainID == 0

// No registration is ever confirmed (i.e., removed from pending)
// if used as invariant and violated, we have an example where a registration
// is removed from pending
def noConfirmed = 
    (L1.length() > 2) implies L1[L1.length() - 2].pendingRegistrations.forall(
        r => L1[L1.length() - 1].pendingRegistrations.contains(r)
    ) 

// ACTIONS


action init = all {
    L1' = List({height : 0,
                newRegistrations: Set(),
                newProof: Set(),
                pendingRegistrations: Set(),
                verifiedProofs: Set(),
                // provenHeight: -1,
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
        epoch: L1.length() / L1EpochSize, //TODO: check and fix
        index: L1[L1.length() - 1].cntRegistrations + envRegs.size()  + 1, // encodes unique registration index
        address: newVal,
        update: power
    }
    envRegs' = envRegs.union(Set(newReg)),
    L1' = L1,
    L2' = L2,
}

pure def makeProof (b: L2Block) : L2BlockProof = 
    {   height: b.height, 
        confirmedRegs: b.receivedRegistrations, 
        forkID: b.chainID,
    }
            
action addL1Block = all {
    nondet someHeights = (-1).to(L2.length() - 1).powerset().oneOf()
    val l = L2.select(b => someHeights.contains(b.height))
    val newproof = l.foldl( Set(), (p, b) => p.union(Set(makeProof(b))))              
    L1' = L1.append(newL1Block(L1[L1.length() - 1], envRegs, newproof)),
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