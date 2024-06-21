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
    
// const numStrands : int // K: number of strands
// const maxMissingProofs: int // P

val numStrands = 3  // K: number of strands
val maxMissingProofs = 2 // P

type Address = str
type Height = int
type Time = int
type ForkID = int

type L2Proof = {
    provenBlock: Height,
    emptyBlocks: Set[Height],
}

type BlockProof = 
    | Proof (L2Proof)
    | None 


type Block = {
    // meta data
    height: Height,
    //forkID: ForkID, 
    txs: bool,    
    proof: BlockProof,
}

type StrandInfo = int -> int

pure def strand(H: Height, K: int) : int = H % K

pure def strandBlocks(c: List[Block], strandID: int, K: int ) : List[Block] =
    c.select(x => strand(x.height,K) == strandID)

pure def last(l) = l[l.length() - 1]


// TODO: comment what it does    
pure def emptySuffix (s: List[Block]) : Set[Height] =
    if (s == List()) Set()
    else
    val proofStart = s.foldl(head(s).height, (s, x) =>  if (x.height > s and x.proof != None) x.height
                                                            else s)
    s   .select(x => x.height > proofStart)
        .foldl(Set(), (s, x) => s.union(Set(x.height)))

pure def newBlockProof (s: List[Block], H: Height, K: int) : BlockProof =
    if (H < K) None
    else 
        val proofStart = s.foldl(head(s).height, (s, x) =>  if (x.height > s and x.proof != None) x.height
                                                            else s)
        val empty = s   .select(x => x.height > proofStart)
                        .foldl(Set(), (s, x) => s.union(Set(x.height)))
        Proof({provenBlock: proofStart, emptyBlocks: empty})


pure def requireProof (H: Height, K: int, strandInfo: StrandInfo, maxMissing: int) : bool =
    strandInfo.get(strand(H, K)) == maxMissing



var chain: List[Block]
var num_missing_proofs: StrandInfo

// Invariants

def transactionNeedsProofInv = 
    chain.last().txs implies chain.last().proof != None

def NotTooManyEmptyBlocksInProofInv = 
    match chain.last().proof {
        | Proof(p) => p.emptyBlocks.size() <= maxMissingProofs
        | None => true
    }


pure def followersProven(h: Block, s: Set[Block]) : bool =
    val relevantHeights = 1.to(maxMissingProofs - 1).map(x => h.height + x * numStrands)
    val relevantBlocks = s.filter(x => x.height.in(relevantHeights))
    (h.proof == None) implies 
    // if the proof is missing, there is a upcoming block in the strand with a proof 
    // (or the height doesn't exist and we are at the end of the strand)
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

// unproven blocks can only exist at the head of a strand.

def OnlyHeadUnprovenInv = 
    0.to(numStrands - 1).forall(st =>
        val sb = chain.strandBlocks(st, numStrands)
        val proven = sb.foldl(Set(), (s, x) => 
            match x.proof {
                | Proof(p) => s.union(p.emptyBlocks).union(Set(p.provenBlock))
                | None => s
            }
        )
        val maxproven = proven.fold(0, (s,x) => if (x > s) x else s)
        val allblocks = sb.foldl(Set(), (s, x) => s.union(Set(x.height)))
        val unproven = allblocks.exclude(proven)
        unproven.forall(u => u >= maxproven) // >= to eliminate initial state with proven being the empty set
    )


// Witness

// An example with empty blocks proven
def emptyBlocksProvenWitness = 
    match chain.last().proof {
        | Proof(p) => p.emptyBlocks == Set()
        | None => true
    }

// An example with a lot of empty blocks proven
def MaxEmptyWitness = 
    match chain.last().proof {
        | Proof(p) => p.emptyBlocks.size() != maxMissingProofs
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
        num_missing_proofs.keys().forall( x =>
            num_missing_proofs.get(x) == maxMissingProofs) 
            //and chain.length() > 22
    )



// State Machine

action init = all {
    chain' = List({
        height: 0,
        //forkID: 0,
        txs: false,
        proof: None
    }),
    num_missing_proofs' = 0.to(numStrands-1).mapBy(x => 0)
}

action step = {
    val newHeight = chain.last().height + 1
    val sBlocks =  strandBlocks(chain, strand(newHeight, numStrands), numStrands)
    val newProof = newBlockProof(   sBlocks,
                                    newHeight, 
                                    numStrands)
    nondet newTxs = Set(true, false).oneOf()
    if (requireProof(newHeight, numStrands, num_missing_proofs, maxMissingProofs))
        all {
        chain' = chain.append({height: newHeight, txs: newTxs, proof: newProof}),
        num_missing_proofs' = num_missing_proofs.set(strand(newHeight, numStrands), 0)
        }
    else
        nondet addedProof = Set(None, newProof).oneOf()
        all{
            
            if (addedProof == None) all {
                chain' = chain.append({height: newHeight, txs: false, proof: None}),
                num_missing_proofs' = num_missing_proofs.set(   strand(newHeight, numStrands), 
                                                                num_missing_proofs.get(strand(newHeight, numStrands)) +1 )
                }
            else all {
                chain' = chain.append({height: newHeight, txs: newTxs, proof: addedProof}),
                num_missing_proofs' = num_missing_proofs.set(strand(newHeight, numStrands), 0)
            }
        }

}

}



