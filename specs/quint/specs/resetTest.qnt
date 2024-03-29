// -*- mode: Bluespec; -*-

module resetTest {
    
import reset (
    L2EpochSize = 5,
    L1EpochSize = 10,
    maxReg = 3,
    minReg = -3,
    ValAddresses = Set("Huey", "Dewey", "Louie")
) as S from "./reset"


run FailedResetTest = 
    S::init
    .then(S::addRegistration)
    .then(S::addL1BlockNoProof)
    .then((2 * S::L1EpochSize).reps(_ => S::addL1BlockNoProof))
    .then(S::reset)
    .expect(S::L2[S::L2.length()-1].chainID == 2 and S::L2.length() == 2)   // first reset happened
    .then((S::L1EpochSize).reps(_ => S::addL1BlockNoProof))
    .then(S::reset)
    .expect(S::L2[S::L2.length()-1].chainID == 3 and S::L2.length() == 2)   // second reset happened
    .then(S::addL1BlockProofWithHeight(1))                                  // now proof gets into L1
    .expect(not(S::existsStale(S::L1[S::L1.length()-1])))                   // no more stale registrations



run simpleTest = 
    S::init
    .then(all{assert(S::L1.length() == 1), S::addL1Block})
    .then(all{assert(S::L1.length() == 2), S::addL1Block})



}