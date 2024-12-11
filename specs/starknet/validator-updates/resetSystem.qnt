// -*- mode: Bluespec; -*-
//
// States and actions for "Starknet Forced Staking Updates"
// Josef Widder, Informal Systems, 2024

module resetSystem {

import resetFunc.* from "./resetFunc"
export resetFunc.*


//
// STATE MACHINE
//
//

var L1: List[L1Block] // this is a simplification and doesn't consider non-finalized Eth blocks
var L2: List[L2Block]
var envRegs: Set[Registration]
var prevDeletedL2blocks: List[L2Block] 

//
// Auxiliary functions to limit ranges to allow verification with Apalache
// see https://apalache.informal.systems/docs/apalache/known-issues.html#integer-ranges-with-non-constant-bounds
//
pure val minL2Height = -1
pure val maxL2Height = 100
pure val maxDeletedL2Height = 100

def SafeL2Range(a,b) = 
    minL2Height.to(maxL2Height).filter(i => a <= i and i <= b)

def SafeDeletedL2Range(a,b) = 
    0.to(maxDeletedL2Height).filter(i => a <= i and i <= b)



//
// Invariants
//

//
// Local L1 invariants
//
pure def validProof (b : L1Block, prev: L1Block) : bool =
    val epochedRegs = b.newRegistrations.map(r => {... r, eth_epoch: L1Epoch(b.time)})
    val pending = prev.unfulfilled_updates.union(epochedRegs)
    val forkID = expectedForkID(prev.l2forkID, pending, b.time)
    match b.newProof {
            | Proof(p) => (proofOK( prev.provenHeight, 
                                    prev.latestProvenL2BlockHash, forkID, pending, p, b.time))
            | None => false
            | Invalid => false
    }

// If a valid proof was verified on L1, then there should be no stale
// updates
def noStaleWithProofInv = 
    (L1.length() > 1) implies ( 
        validProof(L1[L1.length() - 1], L1[L1.length() - 2]) implies
            not(L1.last().existsStaleInBlock())
    )

// provenHeight non-decreasing
def provenHeightMonotonic = 
    (L1.length() > 1) implies ( 
        0.to(L1.length()-2).forall(i => L1[i].provenHeight <= L1[i+1].provenHeight)
    )

// L1 forkID non-decreasing
def L1ForkIDMonotonic = 
    (L1.length() > 1) implies ( 
        0.to(L1.length()-2).forall(i => L1[i].l2forkID <= L1[i+1].l2forkID)
    )

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

// If there is no (valid) proof or the proof is for a lower height, then the proof should
// be rejected (provenHeight should remain unchanged)
def OldProofRejectedInv =
    L1.length() > 1 and 
        match L1.last().newProof {
            | None => true
            | Invalid => true
            | Proof(p) => p.from_height <= L1[L1.length() -2].provenHeight
        }
    implies L1.last().provenHeight == L1[L1.length() -2].provenHeight   

// If there is no (valid) proof or the proof is for a too large height, then the proof should
// be rejected (provenHeight should remain unchanged)
def FutureProofRejectedInv =
    L1.length() > 1 and 
        match L1.last().newProof {
            | None => true
            | Invalid => true
            | Proof(p) => p.from_height > L1[L1.length() -2].provenHeight + 1
        }
    implies L1.last().provenHeight == L1[L1.length() -2].provenHeight   


//
// Local L2 invariants
//

// ForkID on L2 is non-decreasing
def monotonicForkIDInv = (
    L2.length() > 1 implies 
        (L2[L2.length() - 1].forkID >= L2[L2.length() - 2].forkID)
)

// L2 highest staged monotonic
def monotonicStagedSeqInv = 
    (L2.length() > 1) implies ( 
        0.to(L2.length()-2).forall(i => L2[i].highest_staged_seq_num <= L2[i+1].highest_staged_seq_num)
    )

// L2 height strictly monotonic
def strictlyMonotonicHeightInv = 
    (L2.length() > 1) implies ( 
        0.to(L2.length()-2).forall(i => L2[i].height < L2[i+1].height)
    )


// first line important: we only have unstanged registrations which have seq_num
// greater than highest_staged_seq_num
// this ensures that we don't apply a registration twice
def stagedInv = and (
    L2.last().unstagedUpdates.forall(r => r.seq_num > L2.last().highest_staged_seq_num),
    L2.last().stagedUpdates.forall(r => r.seq_num <= L2.last().highest_staged_seq_num)
)


//
// System-level invariants
//

// stored hash is consistent with corresponding L2 Block
def proofStateInv = 
    L1.last().latestProvenL2BlockHash == L2[L1.last().provenHeight]

// L1 never expects a smaller forkID than there is on L2
def forkIDNotSmaller = 
    L1.last().l2forkID >= L2.last().forkID

// L2 is never rolled-back below provenHeight
def finalizationInv = (
    L2.last().height >= L1.last().provenHeight
)

// Auxiliary function
// returns a list of L2 blocks that are proven in the last L1 Block
pure def provenBlocks (l1: List[L1Block], l2: List[L2Block]) : List[L2Block] = 
    l2.slice(l1[l1.length()-2].provenHeight + 1, l1[l1.length()-1].provenHeight + 1)

// All L2 blocks that are proven with one proof on L1, have the same forkID
def oneForkIDperProofInv = 
    L1.length() > 1 implies 
        provenBlocks(L1, L2).toSet().forall(a =>
            provenBlocks(L1, L2).toSet().forall(b => a.forkID == b.forkID))

// L2 chain shouldn't roll back twice one same forkID 
// checks whether in prevDeletedL2blocks there are two subsequent blocks with the same forkID whose
// Heights are not increasing. If this is the case the second what was not a "normal" successor of
// the first one, but is the result of a reset
def atMostOneResetPerForkIDInv = 
    val l = prevDeletedL2blocks
    l.length() > 2 implies
        SafeDeletedL2Range(0,l.length() -2)
            .forall (i => l[i].forkID != l[i+1].forkID or l[i].height <= l[i+1].height)


def noProvenRegistrationsUnfulfilledInv =
    val provenPrefix = L2.select(x => x.height <= L1.last().provenHeight)
    val received_regs = provenPrefix.foldl(Set(), (s,x) => s.union(x.registrations))
    L1.last().unfulfilled_updates.intersect(received_regs) == Set()


//
// Interesting reachable states
//
// These predicates describe interesting states. More precisely, these are 
// negations of predicates that are interesting. We can ask the simulator
// whether these negations are invariants. The simulator is then looking for
// violations of invariants. If it finds one, it reports a counterexample, that
// is, a trace that ends up in a state where the negation of the predicate is
// violated, in other words, where the predicate holds.


// The last block of L1 doesn't contain stale registrations.
// If used as an invariant, then the last block contains stale registrations
def staleWitness = not(L1.last().existsStaleInBlock())

// forkID of last element is 0
// if used as invariant and violated, we have an example with a reset
def resetWitness = L2.last().forkID == 0

// as above, but ensures that before the reset a proof was accepted at L1
def resetAfterProofWitness = or (
    L2.last().forkID == 0,
    L1.last().provenHeight == 0
)

// Witness that a proof for a forked L2 is accepted on L1
// doesn't work with --step "stepNoRegs"
def forkProvedWitness = 
    (L1.length() > 2) implies 
    (L1.last().provenHeight > L1[L1.length() -2].provenHeight implies
        match L1.last().newProof {
            | None => true
            | Invalid => true
            | Proof(p) => p.forkID == 0
    }
    )


// No registration is ever confirmed (i.e., removed from unfulfilled_updates)
// if used as invariant and violated, we have an example where a registration
// is removed from pending
def ConfirmedWitness = 
    (L1.length() > 2) implies 
    L1[L1.length() - 2].unfulfilled_updates.subseteq(L1.last().unfulfilled_updates) 

// fails if incorrect proof is submitted to L1 
// (e.g., if a stale registration is not confirmed)
// it fails if there is a proof in the L1 block but the provenHeight has not increased 
// Use 
// quint run --invariant "ProofNotAcceptedWitness" --seed=0x71e774273d5b9 resetTest.qnt 
// to reproduce for wrong forkID.
def ProofNotAcceptedWitness = 
    and (
        L1.length() > 1, 
        L1.last().newProof != None, 
        L1.last().newProof != Invalid
    )
    implies L1.last().provenHeight > L1[L1.length() -2].provenHeight

def ProofAcceptedWitness = 
    L1.length() > 1 implies L1.last().provenHeight == L1[L1.length() -2].provenHeight

// if it fails I get a scenario where there was a reset on L2 and before a second block
// was added to L2 with the same fork ID, another reset happened
def unsuccessfulResetWitness = 
    prevDeletedL2blocks.length() > 1 implies 
    any {
        prevDeletedL2blocks.last().forkID == prevDeletedL2blocks[prevDeletedL2blocks.length() - 2].forkID,
        prevDeletedL2blocks.last().height != L2.last().height
    }


// reaches a state where the proof submitted to L1 contains an invalid registration
// needs --step "stepWithInvalidRegs" to find a trace
def InvalidRegReachesL1Witness = 
    match L1.last().newProof {
        | None => true
        | Invalid => true
        | Proof(p) => p.confirmedRegs.filter(r => r.starknet_addr == "INVALID").size() == 0
    }


// scenarios shortly before L1 epoch end and shortly after

// trace where in the previous L1 block there where no stale registrations (timed-out 
// unfulfilled registrations), but the unfulfilled registrations from the previous block
// would become stale in the new block (as the time progressed). In this scenario, the proof
// comes in just in time. The registrations actually don't become stale
// This trace does not exists. The problem is that the "not(existsStale(..." condition
// increases the expected forkID, while the proof was constructed for the previous Eth time
// The run lastPossibleL1BlockWitnessRun shows the behavior
def lastPossibleL1BlockWitnessCandidate = 
    (L1.length() > 1 and validProof(L1[L1.length() - 1], L1[L1.length() - 2]))
    implies (or {
        L1[L1.length()-2].existsStaleInBlock(), // registrations in previous block were not stale
        not(existsStale(L1[L1.length()-2].unfulfilled_updates, L1.last().time)), 
                // these registrations would be stale now
        L1.last().existsStaleInBlock(), // no stale registrations after proof was applied
    })
        
// trace where the is a stale registration in the previous block, but then the proof came, and
// in the new block there are no stale registrations
def ProofAfterStaleWitness = 
    (L1.length() > 1 and validProof(L1[L1.length() - 1], L1[L1.length() - 2]))
    implies (or {
            not(L1[L1.length()-2].existsStaleInBlock()), 
            L1.last().existsStaleInBlock(), // no stale registrations after proof was applied
        }
    )

// scenarios with confirmed registrations and depending on their state in L2

// trace where a registration is confirmed but still staged or unstaged.
def unstagedRegConfirmedWitness = 
    match L1.last().newProof {
        | None => true
        | Invalid => true
        | Proof(p) => or {
            ProofAcceptedWitness,            
            p.confirmedRegs.forall(r => not(L2.last().unstagedUpdates.contains(r)) and 
                                        not(L2.last().stagedUpdates.contains(r)) ),
        }
    }

// trace where a registration is confirmed but not in staged or unstaged.
// --seed=0xd46e092e47703 generated a trace with a reset. Stale registrations never go into
// staged or unstaged, but directly into the valset
def processedRegConfirmedWitness = 
    match L1.last().newProof {
        | None => true
        | Invalid => true
        | Proof(p) => or {
            ProofAcceptedWitness,
            p.confirmedRegs.forall(r => L2.last().unstagedUpdates.contains(r) or
                                        L2.last().stagedUpdates.contains(r)),
        }
    }

// trace where a registration is confirmed but not in staged or unstaged, and the last
// L2 block is not due to a fork
def processedRegConfirmedNoForkWitness = 
    match L1.last().newProof {
        | None => true
        | Invalid => true
        | Proof(p) => or {
            ProofAcceptedWitness,
            p.confirmedRegs.forall(r => L2.last().unstagedUpdates.contains(r) or
                                        L2.last().stagedUpdates.contains(r)),
            L2.length() > 1 implies L2.last().forkID != L2[L2.length() - 2].forkID,
        }
    }

// trace where a registration is confirmed but not in staged or unstaged, and there
// is no fork
def processedRegConfirmedNoForkAtAllWitness = 
    match L1.last().newProof {
        | None => true
        | Invalid => true
        | Proof(p) => or {
            ProofAcceptedWitness,
            p.confirmedRegs.forall(r => L2.last().unstagedUpdates.contains(r) or
                                        L2.last().stagedUpdates.contains(r)),
            prevDeletedL2blocks.length() > 0,
        }
    }

// there is a proof with invalid registration, and the proof is rejected.    
// needs --step "stepWithInvalidRegs"
def InvalidRegistrationProofRejectedWitness =
    and (
        L1.length() > 1, 
        match L1.last().newProof {
            | None => false
            | Invalid => false
            | Proof(p) => p.confirmedRegs.contains( { amount: 0, eth_epoch: 0, seq_num: 0, starknet_addr: "INVALID" })
        }
    )
    implies L1.last().provenHeight > L1[L1.length() -2].provenHeight   

// Generates a trace where an unprovable block is dismissed with --step "step"
// fails to do so under quint --step "stepNoRegs"
def UnprovableBlockDismissedWitness = 
    prevDeletedL2blocks.select(x => not(x.provable)).length() == 0

def OldProofRejectedWitness =
    L1.length() > 1 and 
        match L1.last().newProof {
            | None => false
            | Invalid => false
            | Proof(p) => p.from_height <= L1[L1.length() -2].provenHeight 
                            and p.from_height > 1 // to make the witness more interesting
        }
    implies L1.last().provenHeight > L1[L1.length() -2].provenHeight   

def FutureProofRejectedWitness =
    L1.length() > 1 and 
        match L1.last().newProof {
            | None => false
            | Invalid => false
            | Proof(p) => p.from_height > L1[L1.length() -2].provenHeight + 1
        }
    implies L1.last().provenHeight > L1[L1.length() -2].provenHeight   







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
    match makeProof(L2, L1.last().provenHeight + 1, b.height) {
        | Invalid => false
        | None => false
        | Proof(bp) => proofOK (L1.last().provenHeight, 
                                L1.last().latestProvenL2BlockHash,
                                L1.last().l2forkID,
                                L1.last().unfulfilled_updates, 
                                bp,
                                L1.last().time) // whether a proof is accepted is time-dependent
    }


// The following temporal properties below are just here for documentation. Apalache
// has some basic temporal model checking capabilities, but does not support the 
// logic fragment in which these properties are expressed, e.g., alternations of
// temporal operators (i.e., always and eventually), and set quantifieres (forall).
// We should
// either find human arguments to support them, or write some test cases to capture
// intuition. Also, strictly speaking, most of the temporal properties are violated
// as they would require underlying fairness, e.g., that infinitely often a block is
// added to L1 and to L2.

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

// unprovable block leads to fork
temporal UnprovableBlock = always (
    L2.toSet().forall(x => not(x.provable) implies eventually(L2.last().forkID > x.forkID))
)


// For every height there is eventually a finalized block of that height
temporal finalizedLiveness = 
    0.to(maxL2Height).forall(h => eventually(finalized(L2[h])))

def testfinalized(h: Height) : bool =
    L2.length() > h implies finalized(L2[h])

temporal restf = 
    eventually(testfinalized(1))


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
        provable: true,
        //receivedRegistrations: Set(),
    }
    all {
    L1' = List({time : EthStartTime,
                newRegistrations: Set(),
                newProof: None,
                unfulfilled_updates: Set(),
                provenHeight: 0, // initial state proven?
                l2forkID: 0,
                latestProvenL2BlockHash: initialL2Block, // Remark: the initialization seems to need a specific proof
                highest_sub_seq_num: 0,
                }),
    L2' = List(initialL2Block),
    prevDeletedL2blocks' = List(),
    envRegs' = Set(),
    }


action addRegistration = all {
    nondet newVal = ValAddresses.oneOf()
    nondet power = minReg.to(maxReg).oneOf()
    val newReg = {
        eth_epoch: -1, // we put -1 here and set epoch when put into L1 Block
        seq_num: L1.last().highest_sub_seq_num + envRegs.size() + 1, // encodes unique registration index
        starknet_addr: newVal,
        amount: power
    }
    envRegs' = envRegs.union(Set(newReg)),
    L1' = L1,
    L2' = L2,
    prevDeletedL2blocks' = prevDeletedL2blocks,
}

action addL1BlockParams (proof, blocktimeDelta) = all {
    L1' = L1.append(newL1Block(L1.last(), envRegs, proof, blocktimeDelta)),
    L2' = L2,
    envRegs' = Set(),
    prevDeletedL2blocks' = prevDeletedL2blocks,
}


action addL1Block = all {
    nondet someHeight = SafeL2Range(-1, L2.length() - 1).oneOf() 
    nondet blocktimeDelta = 1.to(MaxEthBlockTime).oneOf()
    val newproof = makeProof(L2, L1.last().provenHeight + 1, someHeight)   
        // if someheight < provenHeight this will give Invalid proof
        // this can potentially generate a proof that is valid now, but because blocktimeDelta
        // pushes L1 to the next epoch, the proof will actually not be accepted.
    nondet submProof = Set(newproof, None).oneOf()
    addL1BlockParams(submProof, blocktimeDelta)
}

action addL2Block = all {
    val previously_rcvd_regs = L2.foldl(Set(), (s, x) => s.union(x.registrations)) // to ensure at-most-once
    val potential_regs = L1.last().unfulfilled_updates.exclude(previously_rcvd_regs)
    nondet regs = potential_regs.powerset().oneOf()
        // This doesn't replay confirmed registrations
        // And it doesn't allow to add invalid registrations, that is, ones that are not in L1.
    nondet provable = Set(true, false).oneOf()  
    L2' = L2.append(newL2Block(L2, regs, provable)),
    L1' = L1,
    envRegs' = envRegs,
    prevDeletedL2blocks' = prevDeletedL2blocks,
}

action resetParam (provable) = all {
    if (existsStaleInBlock(L1.last()) and L2.last().forkID != L1.last().l2forkID)
        all{
            L2' = L2.slice(0, L1.last().provenHeight + 1)
                    .append(forkBlock(
                            L2[L1.last().provenHeight], 
                            L1.last().staleRegsInBlock(), 
                            L1.last().provenHeight + 1, 
                            L1.last().l2forkID,
                            provable)), 
            prevDeletedL2blocks' = prevDeletedL2blocks
                                    .concat(L2.slice(L1.last().provenHeight + 1, L2.length())), 
        }
    else
        all {
            L2' = L2,
            prevDeletedL2blocks' = prevDeletedL2blocks,
        }, 
    L1' =  L1,
    envRegs' = envRegs,
}


action reset = all {
    nondet provable = Set(true, false).oneOf()  
    resetParam(provable)
}

action step = any {
    addRegistration,    // adds a registration to the environment. A bit like submitting a transaction
    addL1Block,         // adds a new L1 block, and takes all registrations from the environment
    addL2Block,         // adds a normal L2 block / no reset
    reset               // this is asynchronous now. We should link it to L1 block generation
}

action stepWithPotentiallyOldProofs = any {
    addRegistration,    
    addL1Block,
    addL1BlockWithOldProof,
    addL2Block, 
    reset      
}

action stepWithPotentiallyFutureProofs = any {
    addRegistration,    
    addL1Block,
    addL1BlockWithFutureProof,
    addL2Block, 
    reset      
}



action stepProvableL2BlocksOnly = any {
    addRegistration,    
    addL1Block,         
    nondet regs = L1.last().unfulfilled_updates.powerset().oneOf()
    addL2BlockParams(regs, true), 
    reset               
}

action stepNoRegs = any {
    addL1Block,
    addL2Block,
    reset   
}

//
// Extra actions to drive tests in a fine-grained way
//


// Remark: We could change the encoding so that an action generates a proof and
// stores it in a state, and then later an L1 actions will then try to get this 
// proof onto the chain. At the moment, I don't see what additional behavior we
// can get. Proofs can already expire in the current encod

// for repl and writing tests
action addL1BlockNoProof (blocktimeDelta) = all {             
        addL1BlockParams(None, blocktimeDelta)
}        


val badL2Block = {
    height: -1,
    forkID: -1, 
    registrations: Set(),
    valset: Map(), 
    stagedUpdates: Set(),
    unstagedUpdates: Set(),
    highest_staged_seq_num: -1,
    provable: true, 
}

// Capturing a form of equivovation. We generate a proof that can be validated on L1, 
// but it doesn't capture the L2 history. In particular, we set the confirmed registration
// to the empty set, and put the resulting block to a default bad block. 
// This allows us to generate traces where a bad proof is accepted on L1, and then good
// proofs cannot be submitted on top of it.
action addL1BlockSplitBrainProof = all{
    nondet someHeight = SafeL2Range(L1.last().provenHeight + 1, L2.length() - 1).oneOf() 
    nondet blocktimeDelta = 1.to(MaxEthBlockTime).oneOf()
    val newproof = makeProof(L2, L1.last().provenHeight + 1, someHeight)   
    val submProof = match newproof {
        | None => None
        | Invalid => Invalid
        | Proof(p) => Proof({... p, confirmedRegs: Set(), resultingBlock: badL2Block})
    }
    addL1BlockParams(submProof, blocktimeDelta)
}



// for repl and writing tests
action addL1BlockProofWithHeight(h, blocktimeDelta) = all {
    val newproof = makeProof(L2, L1.last().provenHeight + 1, h)
    addL1BlockParams(newproof, blocktimeDelta)
}        

action addL1BlockWithOldProof = all {
    nondet blocktimeDelta = 1.to(MaxEthBlockTime).oneOf()
    nondet from_height = 0.to(L1.last().provenHeight).oneOf()
    nondet to_height = L1.last().provenHeight.to(L2.length() - 1).oneOf()
    val newproof = makeProof(L2, from_height, to_height)
    addL1BlockParams(newproof, blocktimeDelta)
}        

action addL1BlockWithFutureProof = all {
    // precondition
    L2.length() > 2,
    L1.last().provenHeight+2 <= L2.length() - 1,
    // action
    nondet blocktimeDelta = 1.to(MaxEthBlockTime).oneOf()
    nondet from_height =  (L1.last().provenHeight+2).to(L2.length() - 1).oneOf()
    nondet to_height = from_height.to(L2.length() - 1).oneOf()
    val newproof = makeProof(L2, from_height, to_height)
    addL1BlockParams(newproof, blocktimeDelta)
}        



// for repl and writing tests
action addL2BlockParams (regs: Set[Registration], provable: bool) : bool = all {
    L2' = L2.append(newL2Block(L2, regs, provable)),
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
    addL2BlockParams(Set(inv), true)
}

action stepWithInvalidRegs = any {
    step, 
    addL2BlockInvalidReg
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
                        L1.last().l2forkID, 
                        true)) // TODO: fix from PR
    else
        L2' = L2,
    L1' =  L1,
    envRegs' = envRegs,
    prevDeletedL2blocks' = prevDeletedL2blocks,//
}



}


