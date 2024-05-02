// -*- mode: Bluespec; -*-

// Observations: 
// - is the number of registrations in L2 block a limiting factor for the reset
// TODO: Write properties in terms of valid, decided, invalidated, finalized blocks

// TODO: L1->L2 messaging uses nonce for at-most-once delivery / but delivery might be out of order

// not captured here: Time between block creation and proof on L1 must be big enough to also have proof on L2

// quint run --invariant "noReset" resetTest.qnt 

// Starknet Forced Staking Updates: https://docs.google.com/document/d/1OaYLh9o10DIsGpW0GTRhWl-IJiVyjRsy7UttHs9_1Fw/edit#heading=h.hvyiqqxvuqzo
// Leader and Proof scheduling: https://docs.google.com/document/d/1qjngaq9GoMWa5UJOrTlKNqwi_pI0aHt8iULWD9Gy9hE/edit#heading=h.s8yc63mi87f5


module reset {

type Address = str
type Height = int
type Time = int
type ForkID = int

const L2EpochSize : Height
const L1EpochSize : Time
const EthStartTime : Time
const MaxEthBlockTime: Time
const maxReg : int
const minReg : int
const ValAddresses : Set[Address]


type Registration = {
    eth_epoch: int, // Computed by eth.now / EVE_duration. Should never be 0 as this creates a corner case with the
                    // initial forkID also being 0.
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
    | Invalid   // for modeling ZK checks. to allow executions where invalid proofs are submitted to L1
    | None      // if no proof is submitted


type L1Block = {
    // meta data
    time : Time,
    // transactions
    newRegistrations: Set[Registration],    // Matan: next_update_id used to make reg.index unique
                                            // We don't capture the staking logic in this model
                                            // For reset we only care about registrations and the absence etc.
                                            // This logic is encoded in the environment now
    newProof: L2Proof, 
    // state
    unfulfilled_updates: Set[Registration], 
    provenHeight: Height, 
    l2forkID: int,
    latestProvenL2BlockHash: L2Block, // What happens is that the actual proof submitted to L1 is computed from L2 off-chain
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
    highest_staged_seq_num: int, // we need this explicitly in case the stagedUpdates becomes empty
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
// Auxiliary functions for L1 chain
//

pure def L1Epoch(t: Time) : int =
    t / L1EpochSize

pure def staleRegs (regs: Set[Registration], t: Time) : Set[Registration] =
    regs.filter(r => r.eth_epoch + 2 <= L1Epoch(t))

pure def staleRegsInBlock (b: L1Block) : Set[Registration] =
    staleRegs(b.unfulfilled_updates, b.time)

pure def existsStale (regs: Set[Registration], t: Time) : bool =
    staleRegs(regs, t).size() > 0

pure def existsStaleInBlock (b: L1Block) : bool = 
    existsStale(b.unfulfilled_updates, b.time)

// set the fork ID equal to the EVE of the most recent stale registration
pure def expectedForkID (prevID: int, regs: Set[Registration], t: Time) : int =
    if (not(existsStale(regs, t))) prevID
    else
        staleRegs(regs, t).fold(prevID, (s, x) => if (x.eth_epoch > s) x.eth_epoch else s) // max

// confirmed registrations in proofs up to the proven height
pure def confirmedRegistrations (vp: L2Proof) : Set[Registration] = 
    match vp {
            | None => Set()
            | Invalid => Set()
            | Proof(p) => p.confirmedRegs
    }

//
// Proof verification
//
pure def proofOK (provenHeight: Height, expForkID: int,  regs: Set[Registration], p: L2BlockProof) : bool = 
    and {   p.from_height == provenHeight + 1,
            p.forkID == expForkID,                  // the forkID is right, and
            regs.exclude(p.confirmedRegs) == Set(), // there are no stale registrations left
            p.confirmedRegs.subseteq(regs),         // check for invalid registrations IMPORTANT
    }

//
// N E W   B L O C K
//
pure def newL1Block (prev: L1Block, regs: Set[Registration], proof: L2Proof, delay: Time) : L1Block =
    val newTime = prev.time + delay
    val epochedRegs = regs.map(r => {... r, eth_epoch: L1Epoch(newTime)})
    val pending = prev.unfulfilled_updates.union(epochedRegs)
    val forkID = expectedForkID(prev.l2forkID, pending, newTime)
    val accepted = match proof {
            | Proof(p) =>   if (proofOK(prev.provenHeight, forkID, pending, p)) proof 
                            else None       
            | None => None
            | Invalid => None
    }
    val newProvenHeight = match accepted {
        | Proof(p) => p.to_height
        | None => prev.provenHeight
        | Invalid => prev.provenHeight
    }
    val newBlockHash = match accepted {
        | Proof(p) => p.resultingBlock
        | None => prev.latestProvenL2BlockHash
        | Invalid => prev.latestProvenL2BlockHash
    }
    {   time : newTime, 
        newRegistrations: regs,
        newProof: proof,
        provenHeight: newProvenHeight,
        latestProvenL2BlockHash: newBlockHash,
        l2forkID: forkID, 
        unfulfilled_updates: pending.exclude(accepted.confirmedRegistrations()),
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

pure def completePrefixIdx(r: Set[Registration], highest: int) : int =
    // smallest unproven block - 1
    val existingIdx = 0.to(highest).union(r.fold(Set(), (s, x) => s.union(Set(x.seq_num))))
    // if we have {0,1,2,4,5,8}, complete is {0,1,2}; TODO test
    val complete = existingIdx.filter(i => 0.to(i).subseteq(existingIdx))
    complete.fold(0, (max, i) => if (i > max) i else max)

pure def maxSeqNum (regs: Set[Registration]) : int =
    regs.fold(-1, (s, x) => if (x.seq_num > s) x.seq_num else s)

pure def denseSubset (regs: Set[Registration], highest: int) : Set[Registration] =
    val c = regs.completePrefixIdx(highest)
    regs.filter(r => r.seq_num <= c)
    



//
// Compute an L2 block in the normal case (no reset)
// block is computed on top of the L2 chain, with new registrations
//
pure def newL2Block (chain: List[L2Block], regs: Set[Registration]) : L2Block = 
    val prev = last(chain)
    if (chain.length() % L2EpochSize == 0) // new epoch
        val toStaged = prev.unstagedUpdates.denseSubset(prev.highest_staged_seq_num)
        val highest = if (toStaged == Set()) prev.highest_staged_seq_num else toStaged.maxSeqNum()
        val toUnstaged = prev.unstagedUpdates
                                .exclude(toStaged)
                                .union(regs)
                                .filter(x => x.seq_num > highest)
        {   valset: nextValSet(prev.valset, prev.stagedUpdates),
            stagedUpdates: toStaged, 
            unstagedUpdates: toUnstaged,
            height: chain.length(), 
            forkID: prev.forkID,
            registrations: regs, 
            highest_staged_seq_num: highest,
        }   
    else
        {   valset: prev.valset, 
            stagedUpdates: prev.stagedUpdates, 
            unstagedUpdates: prev.unstagedUpdates
                                    .union(regs)
                                    .filter(x => x.seq_num > prev.highest_staged_seq_num),
            height: chain.length(), 
            forkID: prev.forkID,
            registrations: regs, 
            highest_staged_seq_num: prev.highest_staged_seq_num,
        }



// Function to compute a first block for a new fork
// TODO: test with holes in registrations
// TODO: test with forkBlock that misses stale registrations
// Question: Is it the case that the forkblock must not include any unstale registrations? 
//           The current encoding allows unstale registrations, as long as there are no gaps.
pure def forkBlock (prev: L2Block, regs: Set[Registration], h: Height, fID: ForkID) : L2Block =
    val allRegs = prev.unstagedUpdates.union(prev.stagedUpdates).union(regs)
    val toApplyNow = allRegs.denseSubset(prev.highest_staged_seq_num)
    val highest = if (toApplyNow == Set()) prev.highest_staged_seq_num else toApplyNow.maxSeqNum()
    val toUnstaged = allRegs.exclude(toApplyNow)
    val newValSet = nextValSet(prev.valset, toApplyNow)
    {   height: h, // lastL1.provenHeight + 1, 
        forkID: fID, // lastL1.expectedForkID(), 
        registrations: regs, 
        valset: newValSet,  
        stagedUpdates: Set(),
        unstagedUpdates: toUnstaged, 
        highest_staged_seq_num: highest,
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

var L1: List[L1Block] // this is a super simplification and doesn't consider non-finalized Eth blocks
var L2: List[L2Block]
var envRegs: Set[Registration]
var prevDeletedL2blocks: List[L2Block] 


//
// Invariants
//

def staleInv = not(L1.last().existsStaleInBlock())

pure def validProof (b : L1Block, prev: L1Block) : bool =
    val epochedRegs = b.newRegistrations.map(r => {... r, eth_epoch: L1Epoch(b.time)})
    val pending = prev.unfulfilled_updates.union(epochedRegs)
    val forkID = expectedForkID(prev.l2forkID, pending, b.time)
    match b.newProof {
            | Proof(p) =>   if (proofOK(prev.provenHeight, forkID, pending, p)) true 
                            else false       
            | None => false
            | Invalid => false
    }

def noStaleWithProof = 
    (L1.length() > 1) implies ( 
        validProof(L1[L1.length() - 1], L1[L1.length() - 2]) implies
        L1.last().unfulfilled_updates == Set()
    )

// chainID of last element is 0
// if used as invariant and violated, we have an example with a reset
def noReset = L2.last().forkID == 0

// TODO: This doesn't catch duplicate forks
def growingForkID = (
    L2.length() > 1 implies 
        (L2[L2.length() - 1].forkID >= L2[L2.length() - 2].forkID)
)

// Should fail
// No registration is ever confirmed (i.e., removed from pending)
// if used as invariant and violated, we have an example where a registration
// is removed from pending
def noConfirmed = (
        L1.length() > 2) implies 
            L1[L1.length() - 2].unfulfilled_updates.forall(
                r => L1[L1.length() - 1].unfulfilled_updates.contains(r)
) 

// Should hold
// first line important: we only have unstanged registrations which have smaller seq_num.
// this ensures that we don't apply a registration twice
// TODO test
def stagedInv = all {
    L2.last().unstagedUpdates.forall(r => r.seq_num > L2.last().highest_staged_seq_num),
    L2.last().stagedUpdates.forall(r => r.seq_num <= L2.last().highest_staged_seq_num)
} 


// This returns a list of L2 blocks that are proven in the last L1 Block
pure def provenBlocks (l1: List[L1Block], l2: List[L2Block]) : List[L2Block] = 
    l2.slice(l1[l1.length()-2].provenHeight + 1, l1[l1.length()-1].provenHeight + 1)

// Should hold
// A proof doesn't span L2 heights for more than one forkID
// [ok] No violation found (67417ms).
// Use --seed=0x970fc84d54eb5 to reproduce.
def oneForkIDperProofInv = 
    L1.length() > 1 implies 
        provenBlocks(L1, L2).toSet().forall(a =>
            provenBlocks(L1, L2).toSet().forall(b => a.forkID == b.forkID))
            

// should fail if incorrect proof is submitted to L1
// it fails if there is a proof in the L1 block but the provenHeight has not increased 
// [violation] Found an issue (898ms).
// Use --seed=0x138537f4952eb5 to reproduce.
def allProofsAccepted = 
    L1.length() > 1 and not (L1.last().newProof == None or  L1.last().newProof == Invalid)
        implies L1.last().provenHeight > L1[L1.length() -2].provenHeight

// should hold
// If there is no (valid) proof or the proof contains an invalid registration, then the proof should
// be rejected (provenHeight should remain unchanged)
def InvalidRegistrationProofRejectedInv =
    L1.length() > 1 and 
        match L1.last().newProof {
            | None => true
            | Invalid => true
            | Proof(p) => p.confirmedRegs.contains( { amount: 0, eth_epoch: 0, seq_num: 0, starknet_addr: "INVALID" })
        }
    implies L1.last().provenHeight == L1[L1.length() -2].provenHeight   


// Should fail if transition system allows invalid transactions to be added to L2
// transition relation "Step or addL2BlockInvalidReg"
// there is a proof with invalid registration, and the proof is accepted.    
def InvalidRegistrationProofAccepted =
    L1.length() > 1 and 
    L1.last().newProof != None and 
    L1.last().newProof != Invalid and
    match L1.last().newProof {
        | None => true
        | Invalid => true
        | Proof(p) => p.confirmedRegs.contains( { amount: 0, eth_epoch: 0, seq_num: 0, starknet_addr: "INVALID" })
    }
    implies L1.last().provenHeight == L1[L1.length() -2].provenHeight   

pure def previouslyRolledBackForks (l: List[L2Block]) : Set[int] = 
    l.foldl(Set(), (s,x) => s.union(Set(x.forkID)))
    
// should fail if two resets roll-back the L2
def atMostXRollBacks(x) = 
    previouslyRolledBackForks(prevDeletedL2blocks).size() < x


pure def atMostOneResetPerForkID (l: List[L2Block]) : bool = 
    0.to(l.length() -2).forall (i => l[i].forkID < l[i+1].forkID or l[i].height < l[i+1].height)

// should hold
// L2 chain shouldn't roll back twice one same forkID 
def atMostOneResetPerForkIDInv = 
    atMostOneResetPerForkID(prevDeletedL2blocks)

// should fail
// if it fails I get a scenario where there was a reset on L2 and before a second block
// was added to L2 with the same fork ID, another reset happened
def unsuccessfulReset = 
    prevDeletedL2blocks.length() > 1 implies 
    prevDeletedL2blocks.last().height != L2.last().height

// TODO: scenarios shortly before L1 epoch end and shortly after

// Scenario where the registration is confirmed but still staged or unstaged.

//
// Block properties
// 

def decided (b: L2Block) : bool = 
    L2.select(x => x == b).length() > 0         // b is contained in l2

def finalized (b: L2Block) : bool = 
    decided(b) and L1.last().provenHeight >= b.height // proven

def invalidated (b: L2Block) : bool = 
    decided(b) and     // b is contained in l2
    L1.last().provenHeight < b.height and
    L1.last().l2forkID > b.forkID

def valid (b: L2Block) : bool = 
    match makeProof(L2, b.height, L1) {
        | Invalid => false
        | None => false
        | Proof(bp) => proofOK (L1.last().time, 
                                L1.last().l2forkID,
                                L1.last().unfulfilled_updates, 
                                bp) // whether a proof is accepted is time-dependend
    }



// If a block is finalized it stays finalized (that is, it is never removed from L2)
temporal stableFinalized = always(
    L2.toSet().forall(b => 
        (finalized(b) implies always(finalized(b)))
))

// If a block is invalidated it will eventually not be in L2 anymore
temporal invalidatedRemoved = always(
    L2.toSet().forall(b => 
        (invalidated(b) implies eventually(not(decided(b))))
))

// If a block is invalidated it will never be finalized
temporal invalidatedNeverFinalized = always(
    L2.toSet().forall(b => 
        (invalidated(b) implies always(not(finalized(b))))
))

// If a block is valid then there is an execution where it is finalized (it need not be finalized in all executions)
// should fail to be verified. A counterexample will give a trace where a valid block eventually becomes finalized
temporal validHasNoPathtoFinalized = always(
    L2.toSet().forall(b => 
        (valid(b) implies always(not(finalized(b))))
))

// If a block is valid then eventually it will be invalidated or finalized
temporal validProgress = always(
    L2.toSet().forall(b => 
        (valid(b) implies eventually(finalized(b) or invalidated(b))))
)



// For every height there is eventually a finalized block of that height


//
// ACTIONS
//

action init = 
    val initialL2Block = {
        height: 0,
        forkID: 0,
        registrations: Set(),
        valset: Map(),
        stagedUpdates: Set(),
        unstagedUpdates: Set(),
        highest_staged_seq_num: 0,
        //receivedRegistrations: Set(),
    }
    all {
    L1' = List({time : EthStartTime,
                newRegistrations: Set(),
                newProof: None,
                unfulfilled_updates: Set(),
                provenHeight: 0, // initial state proven?
                l2forkID: 0,
                latestProvenL2BlockHash: initialL2Block,
                cntRegistrations: 0,
                }),
    L2' = List(initialL2Block),
    prevDeletedL2blocks' = List(),
    envRegs' = Set(),
    }


action addRegistration = all {
    nondet newVal = ValAddresses.oneOf()
    nondet power = minReg.to(maxReg).oneOf()
    val newReg = {
        eth_epoch: -1, //TODO: put -1 here and set epoch when put into L1 Block
        seq_num: L1.last().cntRegistrations + envRegs.size() + 1, // encodes unique registration index
        starknet_addr: newVal,
        amount: power
    }
    envRegs' = envRegs.union(Set(newReg)),
    L1' = L1,
    L2' = L2,
    prevDeletedL2blocks' = prevDeletedL2blocks,
}

action addL1Block = all {
    nondet someHeight = (-1).to(L2.length() - 1).oneOf() 
    nondet blocktimeDelta = 1.to(MaxEthBlockTime).oneOf()
    val newproof = makeProof(L2, someHeight, L1)    // if someheight < provenHeight this will give Invalid proof
    val submProof = Set(newproof, None).oneOf()
    L1' = L1.append(newL1Block(L1.last(), envRegs, submProof, blocktimeDelta)),
    L2' = L2,
    envRegs' = Set(),
    prevDeletedL2blocks' = prevDeletedL2blocks,
}

action addL2Block = all {
    nondet regs = L1.last().unfulfilled_updates.powerset().oneOf()  
        // This doesn't replay confirmed registrations
        // And it doesn't ensure at most once semantics for transferring registrations
        // And it doesn't allow to add invalid registrations, that is, ones that are not in L1.
    L2' = L2.append(newL2Block(L2, regs)),
    L1' = L1,
    envRegs' = envRegs,
    prevDeletedL2blocks' = prevDeletedL2blocks,
}

action reset = all {
    if (existsStaleInBlock(L1.last()) and L2.last().forkID != L1.last().l2forkID)
        // TODO debug to make sure L1 last forkID and stale are handled on the right L1 block
        // there is an off by one issue. 
        // AND last block on L2 must have old fork id TODO: important
        all{
            L2' = L2.slice(0, L1.last().provenHeight + 1)
                .append(forkBlock(
                            L2[L1.last().provenHeight], 
                            L1.last().staleRegsInBlock(), 
                            L1.last().provenHeight + 1, 
                            L1.last().l2forkID)), 
                // TODO: write more involved test that fails old_reset
            prevDeletedL2blocks' = prevDeletedL2blocks.concat(
                                L2.slice(L1.last().provenHeight + 1, L2.length())), 
        }
    else
        all {
            L2' = L2,
            prevDeletedL2blocks' = prevDeletedL2blocks,
        }, 
    L1' =  L1,
    envRegs' = envRegs,
    
}

action step = any {
    addRegistration,    // adds a registration to the environment. A bit like submitting a transaction
    addL1Block,         // adds a new L1 block, and takes all registrations from the environment
    addL2Block,         // adds a normal L2 block / no reset
    reset               // this is asynchronous now. We should link it to L1 block generation
}



//
// Extra actions to drive tests in a fine-grained way
//

// for repl and writing tests
action addL1BlockNoProof (blocktimeDelta) = all {             
    L1' = L1.append(newL1Block(L1.last(), envRegs, None, blocktimeDelta)),
    L2' = L2,
    envRegs' = Set(),
    prevDeletedL2blocks' = prevDeletedL2blocks,
}        

// TODO action that generates a proof 
// And the L1 actions will then try to get this proof onto the chain

// for repl and writing tests
action addL1BlockProofWithHeight(h, blocktimeDelta) = all {
    val newproof = makeProof(L2, h, L1)
    L1' = L1.append(newL1Block(L1.last(), envRegs, newproof, blocktimeDelta)),
    L2' = L2,
    envRegs' = Set(),
    prevDeletedL2blocks' = prevDeletedL2blocks,
}        



// for repl and writing tests
action addL2BlockRegs (regs: Set[Registration]) : bool = all {
    L2' = L2.append(newL2Block(L2, regs)),
    L1' = L1,
    envRegs' = envRegs,
    prevDeletedL2blocks' = prevDeletedL2blocks,
}

// adds a registraton to L2 that doesn't come from L1
action addL2BlockInvalidReg = all {
    val inv = {
    eth_epoch: 0,
    seq_num: 0,
    starknet_addr: "INVALID",
    amount: 0,
    }
    addL2BlockRegs(Set(inv))
}

// this was the first version of reset that contains two bugs
// in the if conditions, multiple resets can happen for the same forkID (L2 progress is over-written)
// the forkBlock function used the wrong L2 block as a base to compute the next block
action old_reset = all {
    if (existsStaleInBlock(L1.last()))
        L2' = L2
            .slice(0, L1.last().provenHeight + 1)
            .append(forkBlock(
                        L2.last(), 
                        L1.last().staleRegsInBlock(), 
                        L1.last().provenHeight + 1, 
                        L1.last().l2forkID)) // TODO: fix from PR
    else
        L2' = L2,
    L1' =  L1,
    envRegs' = envRegs,
    prevDeletedL2blocks' = prevDeletedL2blocks,//
}





}