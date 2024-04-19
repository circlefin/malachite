// -*- mode: Bluespec; -*-

// TODO: is the number of registrations in L2 block a limitting factor for the reset
// TODO: generate unsuccesful reset schenario
// TODO: Write properties in terms of valid, decided, invalidated, finalized blocks

// TODO: L1->L2 messaging uses nonce for at-most-once delivery / but delivery might be out of order

// TODO: Time between block creation and proof on L1 must be big enough to also have proof on L2

// quint run --invariant "noReset" reset.qnt 

// Starknet Forced Staking Updates: https://docs.google.com/document/d/1OaYLh9o10DIsGpW0GTRhWl-IJiVyjRsy7UttHs9_1Fw/edit#heading=h.hvyiqqxvuqzo
// Leader and Proof scheduling: https://docs.google.com/document/d/1qjngaq9GoMWa5UJOrTlKNqwi_pI0aHt8iULWD9Gy9hE/edit#heading=h.s8yc63mi87f5


module reset {

type Address = str
type Height = int
type Time = int
type ForkID = int

// val    L2EpochSize = 5
// val    L1EpochSize = 3
// val    maxReg = 3
// val    minReg = -3
// val    ValAddresses = Set("Huey", "Dewey", "Louie")


const L2EpochSize : Height
const L1EpochSize : Time
const EthStartTime : Time
const maxReg : int
const minReg : int
const ValAddresses : Set[Address]

type Registration = {
    eth_epoch: int, // Time/Time gives unit 1
    seq_num: int, 
    starknet_addr: Address, 
    amount: int 
}

type L2BlockProof = {
    from_height: Height,
    to_height: Height,
    confirmedRegs: Set[Registration],
    forkID: ForkID,
    resultingBlock: L2Block, // Block of to_height
}

type L2Proof = 
    | Proof (L2BlockProof)
    | Invalid // for modeling ZK checks. to allow executions where invalid proofs are submitted
    | None


type L1Block = {
    // meta data
    time : Time,
    // transactions
    newRegistrations: Set[Registration], // MAtan: next_update_id used to make reg.index unique
    newProof: L2Proof, 
    // state
    pendingRegistrations: Set[Registration], // TODO: Matan: unfulfilled_updates
    provenHeight: Height, 
    l2forkID: int,
    latestProvenL2BlockHash: L2Block, // What happens is that the actuall proof submitted to L1 is computed from L2 off-chain
    // history variable for analysis. Not sure we actually need it
    cntRegistrations: int,
}

type L2Block = {
    // meta data
    height: int,
    forkID: ForkID, 
    // transactions    
    registrations: Set[Registration],
    // state    
    valset: Address -> int, // stakers
    stagedUpdates: Set[Registration],
    unstagedUpdates: Set[Registration],
    highest_applied_seq_num: int,
    // history of received registrations needed for proof (for analysis. history variable)
    // receivedRegistrations: Set[Registration],
    // proofs not modelled here
}

//
// Auxiliary functions
//
pure def last(l) =
    l[l.length() - 1]

pure def toSet (l) = 
    l.foldl(Set(), (s, x) => s.union(Set(x)))


//
// Functions for L1 chain
//


pure def L1Epoch(b: L1Block) : int =
    b.time / L1EpochSize

pure def staleRegs (b: L1Block) : Set[Registration] =
    b.pendingRegistrations.filter(r => r.eth_epoch + 2 <= L1Epoch(b))

pure def existsStale (b: L1Block) : bool =
    staleRegs(b).size() > 0

// set the fork ID equal to the EVE of the most recent stale registration
pure def expectedForkID (b: L1Block) : int =
    if (not(existsStale(b))) b.l2forkID
    else
        staleRegs(b).fold(b.l2forkID, (s, x) => if (x.eth_epoch > s) x.eth_epoch else s) // max

// confirmed registrations in proofs up to the proven height
pure def confirmedRegistrations (vp: L2Proof) : Set[Registration] = 
    match vp {
            | None => Set()
            | Invalid => Set()
            | Proof(p) => p.confirmedRegs
    }


// If no registrations are stale, I accept everything from past epochs
// otherwise, for heights greater they need to be from the current epoch
pure def proofOK (prev: L1Block, p: L2BlockProof) : bool =
    if (prev.provenHeight + 1 != p.from_height) false
    else
        p.forkID == prev.expectedForkID() // if you "apply proof", as a resykt there cannot be stale registrations left

pure def newL1Block (prev: L1Block, regs: Set[Registration], proof: L2Proof) : L1Block =
    val accepted = match proof {
            | None => None
            | Invalid => None
            | Proof(p) =>        
                if (proofOK(prev, p)) proof
                else None
    }
    val confirmed = confirmedRegistrations(accepted)
    val newProvenHeight = match accepted {
        | None => prev.provenHeight
        | Invalid => prev.provenHeight
        | Proof(p) => p.to_height
    }
    val newBlockHash = match accepted {
        | None => prev.latestProvenL2BlockHash
        | Invalid => prev.latestProvenL2BlockHash
        | Proof(p) => p.resultingBlock
    }
    {   time : prev.time + 1, // TODO: right now time increments. We should pass in the time as a parameter to this function
        newRegistrations: regs,
        newProof: proof,
        provenHeight: newProvenHeight,
        latestProvenL2BlockHash: newBlockHash,
        l2forkID: prev.expectedForkID(), // off by one perhaps.
        pendingRegistrations: prev.pendingRegistrations.union(regs).exclude(confirmed),
        cntRegistrations: prev.cntRegistrations + regs.size()
    }

//
// Functions for L2 chain
//

// auxiliary function to compute new valset. Need to figure out whether validator
// already is in the valset
pure def addOneReg (vals: Address -> int, reg: Registration) : Address -> int =
    if (vals.keys().contains(reg.starknet_addr))
        vals.set(reg.starknet_addr, vals.get(reg.starknet_addr) + reg.amount)
    else
        vals.put(reg.starknet_addr, reg.amount)

pure def nextValSet (vals: Address -> int, regs: Set[Registration]) : Address -> int =
    regs.fold(vals, (v, r) => addOneReg(v,r))

pure def completePrefixIdx(r: Set[Registration]) : int =
    // smallest unproven block - 1
    val existingIdx = Set(0).union(r.fold(Set(), (s, x) => s.union(Set(x.seq_num))))
    // if we have {0,1,2,4,5,8}, complete is {0,1,2}; TODO test
    val complete = existingIdx.filter(i => 0.to(i).subseteq(existingIdx))
    complete.fold(0, (max, i) => if (i > max) i else max)


type DenseResult = {
    idx: int,
    denseRegs: Set[Registration]
}

pure def denseSubset (regs: Set[Registration]) : DenseResult =
    val c = regs.completePrefixIdx()
    {   idx: c,
        denseRegs: regs.filter(r => r.seq_num <= c)
    }


pure def newL2Block (chain: List[L2Block], regs: Set[Registration]) : L2Block = 
    val prev = last(chain)
    if (chain.length() % L2EpochSize == 0) // new epoch
        val toStaged = prev.unstagedUpdates.denseSubset().denseRegs
        val toUnstaged = prev.unstagedUpdates.exclude(toStaged).union(regs)
        {   valset: nextValSet(prev.valset, prev.stagedUpdates),
            stagedUpdates: toStaged, // TODO: "dense subset" and remainder stays in unstaged
            unstagedUpdates: toUnstaged,
            height: chain.length(), 
            forkID: prev.forkID,
            registrations: regs, 
            highest_applied_seq_num: prev.unstagedUpdates.denseSubset().idx,
            //receivedRegistrations: prev.receivedRegistrations.union(regs),
        }   
    else
        {   valset: prev.valset, 
            stagedUpdates: prev.stagedUpdates, 
            unstagedUpdates: prev.unstagedUpdates.union(regs),
            height: chain.length(), 
            forkID: prev.forkID,
            registrations: regs, 
            highest_applied_seq_num: prev.highest_applied_seq_num,
            //receivedRegistrations: prev.receivedRegistrations.union(regs),
        }




// special function to compute a first block for a new fork
// TODO: test with holes in registrations
// TODO: test with forkBlock that misses stale registrations
pure def forkBlock (prev: L2Block, regs: Set[Registration], h: Height, fID: ForkID) : L2Block =
    val allRegs = prev.unstagedUpdates.union(prev.stagedUpdates).union(regs)
    val toApplyNow = allRegs.denseSubset().denseRegs
    val toUnstaged = allRegs.exclude(toApplyNow)
    val newValSet = nextValSet(prev.valset, toApplyNow)
    {   height: h, // lastL1.provenHeight + 1, 
        forkID: fID, // lastL1.expectedForkID(), 
        registrations: regs, 
        valset: newValSet,  
        stagedUpdates: Set(),
        unstagedUpdates: toUnstaged, 
        highest_applied_seq_num: allRegs.denseSubset().idx, 
        //receivedRegistrations: prev.receivedRegistrations.union(regs),
    }



//
// Functions for Provers
//

pure def makeProof (l2: List[L2Block], to_height: Height, l1: List[L1Block]) : L2Proof = 
    val from_height = l1.last().provenHeight + 1
    if (from_height <= to_height and to_height < l2.length())
        val listToProve = l2.slice(from_height, to_height + 1)
        val regs = listToProve.foldl (Set(), (s, x) => s.union(x.registrations))
        val newforkID = listToProve.last().forkID
        Proof({ from_height: from_height,
                to_height: to_height,
                confirmedRegs: regs,
                forkID: newforkID,
                resultingBlock: l2[to_height]})
    else
        Invalid



//
// STATE MACHINE
//
//

var L1: List[L1Block]
var L2: List[L2Block]
var envRegs: Set[Registration]

// Invariants
    
def staleInv = not(L1.last().existsStale())

// chainID of last element is 0
// if used as invariant and violated, we have an example with a reset
def noReset = L2.last().forkID == 0

def growingForkID = (
    L2.length() > 1 implies 
        (L2[L2.length() - 1].forkID >= L2[L2.length() - 2].forkID)
)

// No registration is ever confirmed (i.e., removed from pending)
// if used as invariant and violated, we have an example where a registration
// is removed from pending
def noConfirmed = (
        L1.length() > 2) implies 
            L1[L1.length() - 2].pendingRegistrations.forall(
                r => L1[L1.length() - 1].pendingRegistrations.contains(r)
) 




pure def provenBlocks (l1, l2) = 
    l2.slice(l1[l1.length()-2].provenHeight + 1, l1[l1.length()-1].provenHeight + 1)

def oneForkIDperProof = (
    L1.length() > 1 implies 
        provenBlocks(L1, L2).toSet().forall(a =>
            provenBlocks(L1, L2).toSet().forall(b => a.forkID == b.forkID))
)



// ACTIONS

action init = 
    val initialL2Block = {
        height: 0,
        forkID: 0,
        registrations: Set(),
        valset: Map(),
        stagedUpdates: Set(),
        unstagedUpdates: Set(),
        highest_applied_seq_num: -1,
        //receivedRegistrations: Set(),
    }
    all {
    L1' = List({time : EthStartTime,
                newRegistrations: Set(),
                newProof: None,
                pendingRegistrations: Set(),
                provenHeight: 0, // initial state proven?
                l2forkID: 0,
                latestProvenL2BlockHash: initialL2Block,
                cntRegistrations: 0,
                }),
    L2' = List(initialL2Block),
    envRegs' = Set(),
    }


action addRegistration = all {
    nondet newVal = ValAddresses.oneOf()
    nondet power = minReg.to(maxReg).oneOf()
    val newReg = {
        eth_epoch: L1.last().L1Epoch(), //TODO: check and fix
        seq_num: L1.last().cntRegistrations + envRegs.size() + 1, // encodes unique registration index
        starknet_addr: newVal,
        amount: power
    }
    envRegs' = envRegs.union(Set(newReg)),
    L1' = L1,
    L2' = L2,
}



    

// for repl and writing tests
action addL1BlockNoProof = all {             
    L1' = L1.append(newL1Block(L1.last(), envRegs, None)),
    L2' = L2,
    envRegs' = Set(),
}        

// for repl and writing tests
action addL1BlockProofWithHeight(h) = all {
    val newproof = makeProof(L2, h, L1)
    L1' = L1.append(newL1Block(L1.last(), envRegs, newproof)),
    L2' = L2,
    envRegs' = Set(),
}        

// TODO: rewrite action to take heights 
action addL1Block = all {
    nondet someHeight = (-1).to(L2.length() - 1).oneOf() // TODO: instead of -1 take lastProvenHeight
    val newproof = makeProof(L2, someHeight, L1)              
    L1' = L1.append(newL1Block(L1.last(), envRegs, newproof)),
    L2' = L2,
    envRegs' = Set(),
}

// for repl and writing tests
action addL2BlockRegs (regs: Set[Registration]) : bool = all {
    L2' = L2.append(newL2Block(L2, regs)),
    L1' = L1,
    envRegs' = envRegs,
}

action addL2Block = all {
    nondet regs = L1.last().pendingRegistrations.powerset().oneOf() 
    // TODO: Question: at most once?
    L2' = L2.append(newL2Block(L2, regs)),
    L1' = L1,
    envRegs' = envRegs,
}

// this was the first version of reset that contains two bugs
// in the if conditions, multiple resets can happen for the same forkID (L2 progress is over-written)
// the forkBlock function used the wrong L2 block as a base to compute the next block
action old_reset = all {
    if (existsStale(L1.last()))
        L2' = L2
            .slice(0, L1.last().provenHeight + 1)
            .append(forkBlock(
                        L2.last(), 
                        L1.last().staleRegs(), 
                        L1.last().provenHeight + 1, 
                        L1.last().expectedForkID())) // TODO: fix from PR
    else
        L2' = L2,
    L1' =  L1,
    envRegs' = envRegs,
}

action reset = all {
    if (existsStale(L1.last()) and L2.last().forkID != L1.last().l2forkID)
        // TODO debug to make sure L1 last forkID and stale are handled on the right L1 block
        // there is an off by one issue. 
        // AND last block on L2 must have old fork id TODO: important
        L2' = L2
                .slice(0, L1.last().provenHeight + 1)
                .append(forkBlock(
                            L2[L1.last().provenHeight], 
                            L1.last().staleRegs(), 
                            L1.last().provenHeight + 1, 
                            L1.last().expectedForkID())) 
                // TODO: write more involved test that fails old_reset
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



}