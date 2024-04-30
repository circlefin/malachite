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

var L1: List[L1Block] // this is a super simplification and doesn't consider non-finalized Eth blocks
var L2: List[L2Block]
var envRegs: Set[Registration]
var prevDeletedL2blocks: List[L2Block] 

//
// Auxiliary functions to limit ranges to allow verification with Apalache
// see https://apalache.informal.systems/docs/apalache/known-issues.html#integer-ranges-with-non-constant-bounds
//
val minL2Height = -1
val maxL2Height = 100
val maxDeletedL2Height = 100

def SafeL2Range(a,b) = 
    minL2Height.to(maxL2Height).filter(i => a <= i and i <= b)

def SafeDeletedL2Range(a,b) = 
    0.to(maxDeletedL2Height).filter(i => a <= i and i <= b)



//
// Invariants
//

pure def validProof (b : L1Block, prev: L1Block) : bool =
    val epochedRegs = b.newRegistrations.map(r => {... r, eth_epoch: L1Epoch(b.time)})
    val pending = prev.unfulfilled_updates.union(epochedRegs)
    val forkID = expectedForkID(prev.l2forkID, pending, b.time)
    match b.newProof {
            | Proof(p) =>   if (proofOK(prev.provenHeight, forkID, pending, p, b.time)) true 
                            else false       
            | None => false
            | Invalid => false
    }

// If a valid proof was verified on L1, then there should be no unfullfilled
// updates
def noStaleWithProofInv = 
    (L1.length() > 1) implies ( 
        validProof(L1[L1.length() - 1], L1[L1.length() - 2]) implies
            L1.last().unfulfilled_updates == Set()
    )

// ForkID on L2 is non-decreasing
def monotonicForkIDInv = (
    L2.length() > 1 implies 
        (L2[L2.length() - 1].forkID >= L2[L2.length() - 2].forkID)
)

// L2 is never rolled-back below provenHeight
def finalizationInv = (
    L2.last().height >= L1.last().provenHeight
)

// first line important: we only have unstanged registrations which have seq_num
// greater than highest_staged_seq_num
// this ensures that we don't apply a registration twice
// TODO test
def stagedInv = all {
    L2.last().unstagedUpdates.forall(r => r.seq_num > L2.last().highest_staged_seq_num),
    L2.last().stagedUpdates.forall(r => r.seq_num <= L2.last().highest_staged_seq_num)
} 


// Auxiliary function
// returns a list of L2 blocks that are proven in the last L1 Block
pure def provenBlocks (l1: List[L1Block], l2: List[L2Block]) : List[L2Block] = 
    l2.slice(l1[l1.length()-2].provenHeight + 1, l1[l1.length()-1].provenHeight + 1)

// All blocks that are proven with one proof on L1, have the same forkID
def oneForkIDperProofInv = 
    L1.length() > 1 implies 
        provenBlocks(L1, L2).toSet().forall(a =>
            provenBlocks(L1, L2).toSet().forall(b => a.forkID == b.forkID))

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

// L2 chain shouldn't roll back twice one same forkID 
// checks whether in prevDeletedL2blocks there are two subsequent blocks with the same forkID whose
// Heights are not increasing. If this is the case the second what was not a "normal" successor of
// the first one, but is the result of a reset
def atMostOneResetPerForkIDInv = 
    val l = prevDeletedL2blocks
    l.length() > 2 implies
        SafeDeletedL2Range(0,l.length() -2)
            .forall (i => l[i].forkID != l[i+1].forkID or l[i].height <= l[i+1].height)


//
// Interesting reachable states
//
// These predicates describe interesting states. More precisely, these are 
// negations of predicates that are interesting. We can ask the simulator
// whether these negations are invariants. The simulator is then looking for
// violations of invariants. If it finds one, it reports a counterexample, that
// is, a trace that ends up in a state where the negation of the predicate is
// violated, in other words, where the predicate holds.

// should fail
// The last block of L1 doesn't contain stale registrations.
// If used as an invariant, then the last block contains stale registrations
def staleWitness = not(L1.last().existsStaleInBlock())

// forkID of last element is 0
// if used as invariant and violated, we have an example with a reset
def noResetWitness = L2.last().forkID == 0

// No registration is ever confirmed (i.e., removed from unfulfilled_updates)
// if used as invariant and violated, we have an example where a registration
// is removed from pending
def noConfirmedWitness = (
    L1.length() > 2) implies 
    L1[L1.length() - 2].unfulfilled_updates.subseteq(L1.last().unfulfilled_updates) 

// should fail if incorrect proof is submitted to L1 
// (e.g., if a stale registration is not confirmed)
// it fails if there is a proof in the L1 block but the provenHeight has not increased 
def allProofsAcceptedWitness = 
    and (
        L1.length() > 1, 
        L1.last().newProof != None, 
        L1.last().newProof != Invalid
    )
    implies L1.last().provenHeight > L1[L1.length() -2].provenHeight

// Should fail if transition system allows invalid transactions to be added to L2
// transition relation "Step or addL2BlockInvalidReg"
// there is a proof with invalid registration, and the proof is rejected.    
// TODO: figure out
def InvalidRegistrationProofAcceptedWitness =
    and (
        L1.length() > 1, 
        L1.last().newProof != None, 
        L1.last().newProof != Invalid,
    match L1.last().newProof {
        | None => true
        | Invalid => true
        | Proof(p) => p.confirmedRegs.contains( { amount: 0, eth_epoch: 0, seq_num: 0, starknet_addr: "INVALID" })
    })
    implies L1.last().provenHeight == L1[L1.length() -2].provenHeight   

// if it fails I get a scenario where there was a reset on L2 and before a second block
// was added to L2 with the same fork ID, another reset happened
def unsuccessfulResetWitness = 
    prevDeletedL2blocks.length() > 1 implies 
    prevDeletedL2blocks.last().height != L2.last().height


def InvalidRegReachesL1Witness = 
    match L1.last().newProof {
        | None => true
        | Invalid => true
        | Proof(p) => p.confirmedRegs.filter(r => r.starknet_addr == "INVALID").size() == 0
    }


// TODO: scenarios shortly before L1 epoch end and shortly after

// TODO: Scenario where the registration is confirmed but still staged or unstaged.

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
        | Proof(bp) => proofOK (L1.last().provenHeight, 
                                L1.last().l2forkID,
                                L1.last().unfulfilled_updates, 
                                bp,
                                L1.last().time) // whether a proof is accepted is time-dependend
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
temporal finalizedLiveness = 
    0.to(maxL2Height).forall(h => eventually(finalized(L2[h])))

    



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
    nondet someHeight = SafeL2Range(-1, L2.length() - 1).oneOf() 
    nondet blocktimeDelta = 1.to(MaxEthBlockTime).oneOf()
    val newproof = makeProof(L2, someHeight, L1)    // if someheight < provenHeight this will give Invalid proof
    nondet submProof = Set(newproof, None).oneOf()
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
                        L1.last().l2forkID)) // TODO: fix from PR
    else
        L2' = L2,
    L1' =  L1,
    envRegs' = envRegs,
    prevDeletedL2blocks' = prevDeletedL2blocks,//
}



}


