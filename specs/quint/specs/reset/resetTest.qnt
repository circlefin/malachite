// -*- mode: Bluespec; -*-

module resetTest {
    
import reset (
    L2EpochSize = 5,
    L1EpochSize = 4,
    EthStartTime = 42, // This enforces that the first registrations have EVE greater than the initial forkID
    MaxEthBlockTime = 3,
    maxReg = 3,
    minReg = -3,
    ValAddresses = Set("Huey", "Dewey", "Louie")
).* from "./reset"


run FailedResetTest = 
    init
    .then(addRegistration)
    .then(addL1BlockNoProof(1))
    .then((3 * L1EpochSize).reps(_ => addL1BlockNoProof(1)))
    .then(reset)
    .expect(L2[L2.length()-1].forkID == L1[L1.length()-1].l2forkID and L2.length() == 2)   // first reset happened
    .then((L1EpochSize).reps(_ => addL1BlockNoProof(1)))
    .then(reset) // TODO: this should do nothing as we don't have new registrations between two resets
    // TODO: we need a rule that resets at most once for every forkID
    .expect(L2[L2.length()-1].forkID == L1[L1.length()-1].l2forkID and L2.length() == 2)   // second reset happened
    .then(addL1BlockProofWithHeight(1,1))                                  // now proof gets into L1
    .expect(not(existsStaleInBlock(L1[L1.length()-1])))                   // no more stale registrations


run ResetwithProofonL1Test = 
    init     
    .then(addRegistration)
    .then((3*L1EpochSize).reps(_=>addL1BlockNoProof(1)))
    .then(reset)
    .then(addL1BlockProofWithHeight(1,1))       
    .expect(L2[L2.length()-1] == L1[L1.length()-1].latestProvenL2BlockHash)

run moveOneRegToL1ToL2 = 
    addRegistration
    .then(addL1BlockNoProof(1))
    .then(addL2BlockRegs(L1[L1.length()-1].newRegistrations))


run ProofRegsTest = 
    init     
    .then(5.reps(_=>moveOneRegToL1ToL2))
    .then(addL1BlockProofWithHeight(3,1))

run invalidRegistrationTest =
    init
    .then(moveOneRegToL1ToL2)               // valid registration
    .then(addL2BlockInvalidReg)             // add invalidregistration to L2
    .then(addL1BlockProofWithHeight(2, 1))  // submit proof until height 2
    .expect(L1.last().provenHeight == 0)    // proof rejected /  provenHeight still 0



run simpleTest = 
    init
    .then(all{assert(L1.length() == 1), addL1Block})
    .then(all{assert(L1.length() == 2), addL1Block})



}