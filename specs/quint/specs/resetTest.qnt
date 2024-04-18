// -*- mode: Bluespec; -*-

module resetTest {
    
import reset (
    L2EpochSize = 5,
    L1EpochSize = 4,
    EthStartTime = 42,
    maxReg = 3,
    minReg = -3,
    ValAddresses = Set("Huey", "Dewey", "Louie")
).* from "./reset"


run FailedResetTest = 
    init
    .then(addRegistration)
    .then(addL1BlockNoProof)
    .then((3 * L1EpochSize).reps(_ => addL1BlockNoProof))
    .then(reset)
    .expect(L2[L2.length()-1].forkID == L1[L1.length()-1].l2forkID and L2.length() == 2)   // first reset happened
    .then((L1EpochSize).reps(_ => addL1BlockNoProof))
    .then(reset)
    .expect(L2[L2.length()-1].forkID == L1[L1.length()-1].l2forkID and L2.length() == 2)   // second reset happened
    .then(addL1BlockProofWithHeight(1))                                  // now proof gets into L1
    .expect(not(existsStale(L1[L1.length()-1])))                   // no more stale registrations


run ResetwithProofonL1Test = 
    init
    .then(addRegistration)
    .then((3*L1EpochSize).reps(_=>addL1BlockNoProof))
    .then(reset)
    .then(addL1BlockProofWithHeight(1))       
    .expect(L2[L2.length()-1] == L1[L1.length()-1].latestProvenL2BlockHash)

run simpleTest = 
    init
    .then(all{assert(L1.length() == 1), addL1Block})
    .then(all{assert(L1.length() == 2), addL1Block})



}