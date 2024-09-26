// -*- mode: Bluespec; -*-

module misbehavior {
    
import accTypes.* from "./accTypes"  
import basicSpells.* from "../../Quint/spells/basic"
import extraSpells.* from "../../Quint/spells/extra"

pure def certificateRound (certificate: Set[Vote]) : Round = 
    // assumes certificate is valid (this also ensures that the fold is sound)
    // chooseSome(certificate).round
    certificate.fold(emptyVote, (sum, y) => y).round

pure def certificateValidRound (certificate: Set[Vote]) : Round = 
    // assumes certificate is valid (this also ensures that the fold is sound)
    // chooseSome(certificate).round
    certificate.fold(emptyVote, (sum, y) => y).validRound

pure def certificateHeight (certificate: Set[Vote]) : Height = 
    // assumes certificate is valid (this also ensures that the fold is sound)
    // chooseSome(certificate).round
    certificate.fold(emptyVote, (sum, y) => y).height

pure def certificateValue (certificate: Set[Vote]) : Value = 
    // assumes certificate is valid (this also ensures that the fold is sound)
    //chooseSome(certificate).valueId
    certificate.fold(emptyVote, (sum, y) => y).valueId


pure def totalWeight(validatorSet: ValidatorSet): Weight =
    validatorSet.mapSumValues()

pure def countVotes (votes: Set[Vote], v: ValueId, valset: ValidatorSet) : 
Weight =
    votes.filter(vote => vote.valueId == v).fold(0, (s, x) => s + valset.getOrElse(x.srcAddress, 0))

pure def validCertificate (certificate: Set[Vote], valset: ValidatorSet) : bool = 
    val v = certificate.fold(emptyVote, (sum, y) => y)   // certificate chooseSome()
    all {
        v.valueId != Nil,
        certificate.forall(x => all {   x.voteType == v.voteType,
                                        x.height == v.height,
                                        x.round == v.round,
                                        x.valueId == v.valueId,
                                        x.validRound == v.validRound, //added
                                        }),
        certificate.countVotes(v.valueId, valset) * 3 > 2 * valset.totalWeight(), 
    }

// checks for conflicting prevotes for the same round and returns a set that contains sets
// that contain prevotes from the same process for different values
pure def doublePrevotes (cert1: Certificate, cert2: Certificate, valset: ValidatorSet) : Set[Set[Vote]] =
    val c1 = match cert1 {
            | Polka(c) => c
            | _ => Set()
    }
    val c2 = match cert2 {
            | Polka(c) => c
            | _ => Set()
    }    
    if (all {
        validCertificate(c1, valset),
        validCertificate(c2, valset),
        c1.certificateValue() != c2.certificateValue(),
        c1.certificateHeight() == c2.certificateHeight(),
        c1.certificateRound() == c2.certificateRound(), 
        }) 
        val bad = c1.map(x => x.srcAddress).intersect(c2.map(x => x.srcAddress))
        bad.map(b => c1.filter( x => x.srcAddress == b).union(c2.filter( x => x.srcAddress == b)))
    else Set()

// checks for conflicting votes for different rounds. 
pure def amnesiaVotes (cert1: Certificate, cert2: Certificate, valset: ValidatorSet) : Set[Set[Vote]] =
    val commit = match cert1 {
            | Commit(c) => c
            | _ => Set()
    }
    val polka = match cert2 {
            | Polka(c) => c
            | _ => Set()
    }    
    if (all {
        validCertificate(commit, valset),
        validCertificate(polka, valset),
        commit.certificateValue() != polka.certificateValue(),
        commit.certificateHeight() == polka.certificateHeight(),
        commit.certificateRound() < polka.certificateRound(),
        commit.certificateRound() > polka.certificateValidRound(),
        })  
        val bad = commit.map(x => x.srcAddress).intersect(polka.map(x => x.srcAddress))
        bad.map(b => commit.filter( x => x.srcAddress == b).union(polka.filter( x => x.srcAddress == b)))
    else Set()



run detectableTest =
    val validatorSet =  Map("v1" -> 1, "v2" -> 1, "v3" -> 1, "v4" -> 1)
    val polkaBlue = Polka(Set(
        { height: 0, round: 10, srcAddress: "v2", valueId: Val("blue"), voteType: Prevote, validRound: 5 },
        { height: 0, round: 10, srcAddress: "v3", valueId: Val("blue"), voteType: Prevote, validRound: 5 },
        { height: 0, round: 10, srcAddress: "v4", valueId: Val("blue"), voteType: Prevote, validRound: 5 }        
    ))
    val polkaRed = Polka(Set(
        { height: 0, round: 10, srcAddress: "v2", valueId: Val("red"), voteType: Prevote, validRound: 5 },
        { height: 0, round: 10, srcAddress: "v3", valueId: Val("red"), voteType: Prevote, validRound: 5 },
        { height: 0, round: 10, srcAddress: "v1", valueId: Val("red"), voteType: Prevote, validRound: 5 }        
    ))    
    val commitBlue = Commit(Set(
        { height: 0, round: 6, srcAddress: "v2", valueId: Val("blue"), voteType: Precommit, validRound: -1 },
        { height: 0, round: 6, srcAddress: "v3", valueId: Val("blue"), voteType: Precommit, validRound: -1 },
        { height: 0, round: 6, srcAddress: "v4", valueId: Val("blue"), voteType: Precommit, validRound: -1 }        
    ))
     
    all{
        assert(doublePrevotes(polkaBlue, polkaRed, validatorSet) == Set(
            Set(
                { height: 0, round: 10, srcAddress: "v2", valueId: Val("blue"), voteType: Prevote, validRound: 5 },
                { height: 0, round: 10, srcAddress: "v2", valueId: Val("red"), voteType: Prevote, validRound: 5 }
            ),
            Set(
                { height: 0, round: 10, srcAddress: "v3", valueId: Val("blue"), voteType: Prevote, validRound: 5 },
                { height: 0, round: 10, srcAddress: "v3", valueId: Val("red"), voteType: Prevote, validRound: 5 }
            )
        )),
        assert(amnesiaVotes(commitBlue, polkaRed, validatorSet) == Set(
            Set(
                { height: 0, round: 6, srcAddress: "v2", valueId: Val("blue"), voteType: Precommit, validRound: -1 },
                { height: 0, round: 10, srcAddress: "v2", valueId: Val("red"), voteType: Prevote, validRound: 5 }
            ),
            Set(
                { height: 0, round: 6, srcAddress: "v3", valueId: Val("blue"), voteType: Precommit, validRound: -1 },
                { height: 0, round: 10, srcAddress: "v3", valueId: Val("red"), voteType: Prevote, validRound: 5 }
            )
        )),    
    }


}
