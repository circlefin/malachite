module reset {

type Address = str

type Validator = {
    address : Address,
    power: int
}

type Registration = {
    epoch: int,
    index: int,
    address: Address,
    update: int
}

type Proof = 
    | L2Proof((int,Set[Registration]))
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
    // transactions    
    registrations: Set[Registration],
    // state    
    valset: Set[Validator],
    nextValset: Set[Validator],
    pendingValsetUpdates: Set[Registration],
    // history of received registrations needed for proof
    receivedRegistrations: Set[Registration],
}

var L1: List[L1Block]


var L2: List[L2Block]

val L2EpochSize = 5
val L1EpochSize = 3
val maxReg = 3
val minReg = -3

val ValAddresses = Set("Huey", "Dewey", "Louie")

action init = all {
    L1' = List({height : 0,
                newRegistrations: Set(),
                proof: Nil,
                pendingRegistrations: Set(),
                provenHeight: -1,
                cntRegistrations: 0,
                }),
    L2' = List({height: 0,
                registrations: Set(),
                valset: Set({address: "Donald", power: 1}),
                nextValset: Set({address: "Donald", power: 1}), // Question: initialized?
                pendingValsetUpdates: Set(),
                receivedRegistrations: Set(),
                }),
}

pure def updatePending (new: Set[Registration], pending: Set[Registration], confirmed: Set[Registration]) : Set[Registration] =
    pending.union(new).exclude(confirmed)

pure def confirmedRegs (proof: Proof)  : Set[Registration] =
    match proof {
        | Nil => Set()
        | L2Proof(p) => p._2
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
    {   height : chain.length(),
        newRegistrations: regs,
        proof: newProof,
        pendingRegistrations: updatePending(regs, prev.pendingRegistrations, confirmedRegs(proof)),
        provenHeight: ph,
        cntRegistrations: prev.cntRegistrations + 1 // TODO: properly handly multiple regs per block
    } 

action addL1Block = all {
    nondet newWal = ValAddresses.oneOf()
    nondet power = minReg.to(maxReg).oneOf()
    nondet proofHeight = (-1).to(L2.length() - 1).oneOf()
    val proof = if (proofHeight == -1) Nil
                else L2Proof((proofHeight, L2[proofHeight].receivedRegistrations)) 
    val newReg = {
        epoch: L1.length() / L1EpochSize,
        index: L1[L1.length() - 1].cntRegistrations + 1,
        address: newWal,
        update: power
    }
    L1' = L1.append(newL1Block(L1, Set(newReg), proof)),
    L2' = L2,
}

pure def updatePendingUpdates (pending: Set[Registration], regs: Set[Registration]) : Set[Registration] =
    pending.union(regs)

pure def nextValSet (vals: Set[Validator], regs: Set[Registration]) : Set[Validator] =
    // TODO implement update
    vals

pure def newL2Block (chain: List[L2Block], regs: Set[Registration]) : L2Block = 
    val prev = chain[chain.length() - 1]
    if (chain.length() % L2EpochSize == 0) // new epochj
        {   height: chain.length(), 
            registrations: regs, 
            valset: prev.nextValset, 
            nextValset: nextValSet( prev.nextValset, 
                                    updatePendingUpdates(prev.pendingValsetUpdates, regs)),
            pendingValsetUpdates: Set(),
            receivedRegistrations: prev.receivedRegistrations.union(regs),
        }   
    else
        {   height: chain.length(), 
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
}

// action pick registrations to be put into block
// fuction produceL2Block that does valset and registration logic
// action addblock calling function

}