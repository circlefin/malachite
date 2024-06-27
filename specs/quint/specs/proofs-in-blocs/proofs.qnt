// -*- mode: Bluespec; -*-

// This is a protocol specification of "Leader & Proof Scheduling", 
// targeting writing invariants and temporal properties and allowing
// us to generate interesting scenarios.
//
// For details of "Leader & Proof Scheduling", see:
// https://docs.google.com/document/d/1qjngaq9GoMWa5UJOrTlKNqwi_pI0aHt8iULWD9Gy9hE/edit
//
// Josef Widder, Informal Systems, 2024

module proofs {

type Height = int


// this models a recursive proof that consists of the proof of the 
// last block in the strand with a content and then proofs of
// potentially some following empty blocks in the same strand
type RecProof = {
    provenBlock: Height,
    emptyBlocks: Set[Height],
}

type Proof = 
    | L2Proof (RecProof)
    | None 


type Block = {
    // meta data
    height: Height,
    //forkID: ForkID, 
    payload: bool,    
    proof: Proof,
}

type StrandInfo = int -> int

pure def strand(H: Height, K: int) : int = H % K

pure def strandBlocks(c: List[Block], strandID: int, K: int ) : List[Block] =
    c.select(x => strand(x.height,K) == strandID)

pure def last(l) = l[l.length() - 1]


pure def newBlockProof (s: List[Block], H: Height, K: int) : Proof =
    if (H < K) None
    else 
        // the latest block, i.e., the one with the highest height, in the strand that includes a proof.
        val proofStart = s.foldl(head(s).height, (s, x) =>  if (x.height > s and x.proof != None) x.height
                                                            else s)
        // collect all empty blocks in the strand with height > proofStart height
        val empty = s   .select(x => x.height > proofStart)
                        .foldl(Set(), (s, x) => s.union(Set(x.height)))
        L2Proof({provenBlock: proofStart, emptyBlocks: empty})


pure def requireProof (H: Height, K: int, strandInfo: StrandInfo, maxMissing: int) : bool =
    strandInfo.get(strand(H, K)) == maxMissing



// auxiliary functions

// returns heights of blocks at the end of a strand that are not proven    
pure def emptySuffix (s: List[Block]) : Set[Height] =
    if (s == List()) Set()
    else
    // biggest block with proof
    val proofStart = s.foldl(head(s).height, (s, x) =>  if (x.height > s and x.proof != None) x.height
                                                            else s)
    // collect heights of all blocks after that proof
    s   .select(x => x.height > proofStart)
        .foldl(Set(), (s, x) => s.union(Set(x.height)))



val numStrands = 3  // K: number of strands
val maxMissingProofs = 2 // P

var chain: List[Block]
var num_unproven_blocks: StrandInfo

// Invariants


// If a block contains transactions then it is at the very beginning, or the block also contains a proof
def payloadImpliesProofInv = 
    chain.last().payload implies chain.last().height < numStrands or chain.last().proof != None

// The number of proofs of empty blocks in a recursive proof is bounded by maxMissingProofs
def NotTooManyEmptyBlocksInProofInv = 
    match chain.last().proof {
        | L2Proof(p) => p.emptyBlocks.size() <= maxMissingProofs
        | None => true
    }

// there is a proof in block h, or
// if the proof is missing, there is a upcoming block in the strand with a proof 
// (or the height doesn't exist and we are at the end of the strand)
pure def followersProven(h: Block, s: Set[Block]) : bool =
    val relevantHeights = 1.to(maxMissingProofs - 1).map(x => h.height + x * numStrands)
    val relevantBlocks = s.filter(x => x.height.in(relevantHeights))
    (h.proof == None) implies 
        (
            relevantBlocks.size() < maxMissingProofs or
            not(relevantBlocks.forall(b => b.proof == None))
        )
       
def NotTooManyEmptyBlocksInChainInv =
    0.to(numStrands - 1).forall(st =>
        val sb = chain  .select(x => (x.height % numStrands) == st)
                        .foldl(Set(), (s, x) => s.union(Set(x)))
        sb.forall(h => followersProven(h, sb))
    )


// Unproven blocks can only exist at the head of a strand.
def OnlyHeadUnprovenInv = 
    0.to(numStrands - 1).forall(st =>
        val sb = chain.strandBlocks(st, numStrands)
        val proven = sb.foldl(Set(), (s, x) => 
            match x.proof {
                | L2Proof(p) => s.union(p.emptyBlocks).union(Set(p.provenBlock))
                | None => s
            }
        )
        val maxproven = proven.fold(0, (s,x) => if (x > s) x else s)
        val allblocks = sb.foldl(Set(), (s, x) => s.union(Set(x.height)))
        val unproven = allblocks.exclude(proven)
        unproven.forall(u => u >= maxproven) // >= to eliminate initial state with proven being the empty set
    )

// missing_proofs counter is bounded
def missingProofsInv = 
    0.to(numStrands -1).forall(s => num_unproven_blocks.get(s) <= maxMissingProofs)

// number of empty blocks at the head of strand s match num_unproven_blocks.get(s)
def missingProofsAtEndMatchNumInv = 
    0.to(numStrands - 1).forall(st =>
        val sb = chain.strandBlocks(st, numStrands)
        val proven = sb.foldl(Set(), (s, x) => 
            match x.proof {
                | L2Proof(p) => s.union(p.emptyBlocks).union(Set(p.provenBlock))
                | None => s
            }
        )
        val allblocks = sb.foldl(Set(), (s, x) => s.union(Set(x.height)))
        val unproven = allblocks.exclude(proven)
        unproven.size() == num_unproven_blocks.get(st)
    )



// Witness

// An example with empty blocks proven
def emptyBlocksProvenWitness = 
    match chain.last().proof {
        | L2Proof(p) => p.emptyBlocks == Set()
        | None => true
    }

// An example with a lot of empty blocks proven
def MaxEmptyWitness = 
    match chain.last().proof {
        | L2Proof(p) => p.emptyBlocks.size() != maxMissingProofs
        | None => true
    }

// a proof with maximal empty blocks is added
def addMaximalProofWitness = 
    val s = strand(chain.last().height, numStrands)
    val bs = strandBlocks(chain.slice(0, chain.length()-1), s, numStrands)
    bs.emptySuffix().size() == maxMissingProofs implies
        chain.last().proof == None


// Found an issue (7143ms).
// Use --seed=0xdc7a687c5dae7 to reproduce.
def ManyProofsInARowMissingWitness =
    not ( 
        num_unproven_blocks.keys().forall( x =>
            num_unproven_blocks.get(x) == maxMissingProofs) 
            //and chain.length() > 22
    )



// State Machine

action init = all {
    chain' = List({
        height: 0,
        //forkID: 0,
        payload: false,
        proof: None
    }),
    num_unproven_blocks' = 0.to(numStrands-1).mapBy(x => 0).set(0, 1)   // height 0 is unproven, all other 
                                                                        // strands don't have blocks yet
}

action step = {
    val newHeight = chain.last().height + 1
    val sBlocks =  strandBlocks(chain, strand(newHeight, numStrands), numStrands)
    val newProof = newBlockProof(   sBlocks,
                                    newHeight, 
                                    numStrands)
    nondet hasPayload = Set(true, false).oneOf()
    if (requireProof(newHeight, numStrands, num_unproven_blocks, maxMissingProofs))
        all {
        chain' = chain.append({height: newHeight, payload: hasPayload, proof: newProof}),
        num_unproven_blocks' = num_unproven_blocks.set(strand(newHeight, numStrands), 1)
        }
    else
        nondet addedProof = Set(None, newProof).oneOf()
        all{
            
            if (addedProof == None) all {
                val hasPayloadWithoutProof = if (newHeight < numStrands) hasPayload else false
                chain' = chain.append({height: newHeight, payload: hasPayloadWithoutProof, proof: None}),
                num_unproven_blocks' = num_unproven_blocks.set( strand(newHeight, numStrands), 
                                                                num_unproven_blocks.get(strand(newHeight, numStrands)) + 1)
                }
            else all {
                chain' = chain.append({height: newHeight, payload: hasPayload, proof: addedProof}),
                num_unproven_blocks' = num_unproven_blocks.set(strand(newHeight, numStrands), 1)
            }
        }

}

}



