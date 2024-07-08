// -*- mode: Bluespec; -*-
//
// This is a protocol specification of "Starknet Forced Staking Updates", 
// targeting writing invariants and temporal properties and allowing
// us to generate interesting reset scenarios.
//
// For details of "Starknet Forced Staking Updates", see:
// https://docs.google.com/document/d/1OaYLh9o10DIsGpW0GTRhWl-IJiVyjRsy7UttHs9_1Fw/edit#heading=h.hvyiqqxvuqzo
//
// Josef Widder, Informal Systems, 2024


module resetFunc {

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
    baseBlock: L2Block,         // Block of from_height - 1
    resultingBlock: L2Block,    // Block of to_height
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
    highest_sub_seq_num: int,
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
    provable: bool, // this models that something went wrong on block production
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
pure def proofOK (  provenHeight: Height, 
                    baseBlockHash: L2Block,
                    expForkID: int, 
                    regs: Set[Registration], 
                    p: L2BlockProof,
                    t: Time) : bool = 
    and {   p.from_height == provenHeight + 1,
            p.forkID == expForkID,                  // the forkID is right, and
            not(existsStale(regs.exclude(p.confirmedRegs), t)), // there are no stale registrations left
            p.confirmedRegs.subseteq(regs),         // check for invalid registrations IMPORTANT
            p.baseBlock == baseBlockHash,
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
            | Proof(p) =>   if (proofOK(prev.provenHeight, prev.latestProvenL2BlockHash, forkID, pending, p, newTime)) proof 
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
        newRegistrations: epochedRegs,
        newProof: proof,
        provenHeight: newProvenHeight,
        latestProvenL2BlockHash: newBlockHash,
        l2forkID: forkID, 
        unfulfilled_updates: pending.exclude(accepted.confirmedRegistrations()),
        highest_sub_seq_num: prev.highest_sub_seq_num + regs.size()
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


pure def maxSeqNum (regs: Set[Registration]) : int =
    regs.fold(-1, (s, x) => if (x.seq_num > s) x.seq_num else s)


pure def completePrefixIdx(r: Set[Registration], highest: int) : int =
    val existingIdx = r.fold(Set(), (s, x) => s.union(Set(x.seq_num))).union(Set(highest))
    val noSuccessor = existingIdx.filter(x => not(existingIdx.contains(x+1)))
    val largest = noSuccessor.fold(0, (s,x) => if (x>s) x else s)
    noSuccessor.fold(largest, (s,x) => if (x < s) x else s)

pure def denseSubset (regs: Set[Registration], highest: int) : Set[Registration] =
    val c = regs.completePrefixIdx(highest)
    regs.filter(r => r.seq_num <= c)
    



//
// Compute an L2 block in the normal case (no reset)
// block is computed on top of the L2 chain, with new registrations
//
pure def newL2Block (chain: List[L2Block], regs: Set[Registration], provable: bool) : L2Block = 
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
            provable: provable,
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
            provable: provable,
        }



// Function to compute a first block for a new fork
// TODO: test with holes in registrations
// TODO: test with forkBlock that misses stale registrations
// Question: Is it the case that the forkblock must not include any unstale registrations? 
//           The current encoding allows unstale registrations, as long as there are no gaps.
pure def forkBlock (prev: L2Block, regs: Set[Registration], h: Height, fID: ForkID, provable: bool) : L2Block =
    val allRegs = prev.unstagedUpdates.union(prev.stagedUpdates).union(regs) 
        // we clarified this point with Matan on the Google doc, that initially only talked about 
        // staged and stale, but didn't consider unstaged
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
        provable: provable,
    }



//
// Functions for Provers
//

pure def makeProof (l2: List[L2Block], from_height: Height, to_height: Height) : L2Proof = 
    if (from_height <= to_height and to_height < l2.length())
        val listToProve = l2.slice(from_height, to_height + 1)
        val provable = listToProve.foldl(true, (s, x) => s and x.provable)
        if (not(provable)) None
        else
            val regs = listToProve.foldl (Set(), (s, x) => s.union(x.registrations)) // collect all registrations
            val newforkID = listToProve.last().forkID
            Proof({ from_height: from_height,
                    to_height: to_height,
                    confirmedRegs: regs,
                    forkID: newforkID,
                    baseBlock: l2[from_height - 1],
                    resultingBlock: l2[to_height]})
    else
        Invalid

}