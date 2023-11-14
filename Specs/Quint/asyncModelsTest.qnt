// -*- mode: Bluespec; -*-

module asyncModelsTest {

import statemachineAsync(
    validators = Set("v1", "v2", "v3", "v4"),
    validatorSet = Set("v1", "v2", "v3", "v4").mapBy(x => 1),
    Faulty = Set(),
    Values = Set("a", "b"),
    Rounds = Set(0, 1, 2, 3),
    Heights = Set(0) // , 1, 2, 3)
) as N4F0 from "./statemachineAsync"

run NewRoundTest = {
    N4F0::init
    .then(N4F0::setNextValueToPropose("v2", "block"))
    .then(N4F0::valStep("v1"))
    .then(N4F0::valStep("v2"))
    .then(N4F0::valStep("v3"))
    .then(N4F0::valStep("v4"))
    .then(N4F0::valStep("v1"))
    .then(N4F0::deliverProposal("v2", { height: 0, proposal: "block", round: 0, src: "v2", validRound: -1 }))
    .then(N4F0::valStep("v2"))
    .then(N4F0::valStep("v3")) 
    .then(N4F0::deliverVote("v1", { height: 0, id: "nil", round: 0, src: "v1", step: "Prevote" }))
    .then(N4F0::deliverVote("v1", { height: 0, id: "block", round: 0, src: "v2", step: "Prevote" }))
    .then(N4F0::deliverVote("v1", { height: 0, id: "nil", round: 0, src: "v3", step: "Prevote" }))
    .then(N4F0::deliverVote("v2", { height: 0, id: "nil", round: 0, src: "v1", step: "Prevote" }))
    .then(N4F0::deliverVote("v2", { height: 0, id: "block", round: 0, src: "v2", step: "Prevote" }))
    .then(N4F0::deliverVote("v2", { height: 0, id: "nil", round: 0, src: "v3", step: "Prevote" }))
    .then(N4F0::deliverVote("v3", { height: 0, id: "nil", round: 0, src: "v1", step: "Prevote" }))
    .then(N4F0::deliverVote("v3", { height: 0, id: "block", round: 0, src: "v2", step: "Prevote" }))
    .then(N4F0::deliverVote("v3", { height: 0, id: "nil", round: 0, src: "v3", step: "Prevote" }))   
    .then(N4F0::valStep("v1"))
    .then(N4F0::valStep("v2"))
    .then(N4F0::valStep("v3")) 
    .then(N4F0::valStep("v1"))
    .then(N4F0::valStep("v2"))
    .then(N4F0::valStep("v3")) 
    .then(N4F0::valStep("v1"))
    .then(N4F0::valStep("v2"))
    .then(N4F0::valStep("v3")) 
    // timeoutPrevote started
    .then(N4F0::valStep("v1"))
    .then(N4F0::valStep("v2"))
    .then(N4F0::valStep("v3")) 
    .then(N4F0::valStep("v1"))
    .then(N4F0::valStep("v2"))
    .then(N4F0::valStep("v3")) 
    .then(N4F0::deliverVote("v1", { height: 0, id: "nil", round: 0, src: "v1", step: "Precommit" }))
    .then(N4F0::deliverVote("v1", { height: 0, id: "nil", round: 0, src: "v2", step: "Precommit" }))
    .then(N4F0::deliverVote("v1", { height: 0, id: "nil", round: 0, src: "v3", step: "Precommit" }))
    .then(N4F0::deliverVote("v2", { height: 0, id: "nil", round: 0, src: "v1", step: "Precommit" }))
    .then(N4F0::deliverVote("v2", { height: 0, id: "nil", round: 0, src: "v2", step: "Precommit" }))
    .then(N4F0::deliverVote("v2", { height: 0, id: "nil", round: 0, src: "v3", step: "Precommit" }))
    .then(N4F0::deliverVote("v3", { height: 0, id: "nil", round: 0, src: "v1", step: "Precommit" }))
    .then(N4F0::deliverVote("v3", { height: 0, id: "nil", round: 0, src: "v2", step: "Precommit" }))
    .then(N4F0::deliverVote("v3", { height: 0, id: "nil", round: 0, src: "v3", step: "Precommit" }))
    .then(N4F0::valStep("v1"))
    .then(N4F0::valStep("v2"))
    .then(N4F0::valStep("v3")) 
    .then(N4F0::valStep("v1"))
    .then(N4F0::valStep("v2"))
    .then(N4F0::valStep("v3")) 
    .then(N4F0::valStep("v1"))
    .then(N4F0::valStep("v2"))
    .then(N4F0::valStep("v3"))    
    .then(N4F0::valStep("v1"))
    .then(N4F0::valStep("v2"))
    .then(N4F0::valStep("v3"))     
  //  .then(N4F0::deliverProposal("v1",  { height: 0, proposal: "block", round: 1, src: "v2", validRound: -1 }))
/*     .then(N4F0::deliverProposal("v1", { height: 0, proposal: "block", round: 0, src: "v2", validRound: -1 }))
    .then(N4F0::deliverProposal("v2", { height: 0, proposal: "block", round: 0, src: "v2", validRound: -1 }))
    .then(N4F0::deliverProposal("v3", { height: 0, proposal: "block", round: 0, src: "v2", validRound: -1 }))
    .then(N4F0::valStep("v1"))
    .then(N4F0::valStep("v2"))
    .then(N4F0::valStep("v3")) 
  
          */              
}

import statemachineAsync(
    validators = Set("v1", "v2", "v3", "v4"),
    validatorSet = Set("v1", "v2", "v3", "v4").mapBy(x => 1),
    Faulty = Set("v1"),
    Values = Set("a", "b"),
    Rounds = Set(0, 1, 2, 3),
    Heights = Set(0) // , 1, 2, 3)
) as N4F1 from "./statemachineAsync"

// quint run --init=N4F1::init --step=N4F1::step --invariant=N4F1::AgreementInv AsyncModels.qnt
// quint run --init=N4F1::init --step=N4F1::step --invariant=N4F1::ConsensusOutputInv AsyncModels.qnt

import statemachineAsync(
    validators = Set("v1", "v2", "v3", "v4"),
    validatorSet = Set("v1", "v2", "v3", "v4").mapBy(x => 1),
    Faulty = Set("v1", "v2"),
    Values = Set("a", "b"),
    Rounds = Set(0),  // , 1, 2, 3)
    Heights = Set(0) // , 1, 2, 3)
) as N4F2 from "./statemachineAsync"

// v3 and v4 are correct. v2 is a N4N4F22aulty proposal and proposes diN4N4F22N4N4F22erently to v3 and v4
// this run leads to disagreement
run DisagreementTest = {
    N4F2::init
    .then(N4F2::valStep("v3"))
    .then(N4F2::valStep("v4"))
    .then(N4F2::deliverProposal("v3", { height: 0, proposal: "b", round: 0, src: "v2", validRound: -1 }))
    .then(N4F2::deliverProposal("v4", { height: 0, proposal: "a", round: 0, src: "v2", validRound: -1 }))
    .then(N4F2::valStep("v3"))
    .then(N4F2::valStep("v4"))
    .then(all{
        // they voted diN4F2N4F2erently
        assert(N4F2::voteBuffer == Map(
            "v3" -> Set({ height: 0, id: "a", round: 0, src: "v4", step: "Prevote" },  
                        { height: 0, id: "b", round: 0, src: "v3", step: "Prevote" }),
            "v4" -> Set({ height: 0, id: "a", round: 0, src: "v4", step: "Prevote" }, 
                        { height: 0, id: "b", round: 0, src: "v3", step: "Prevote" }))),
            N4F2::deliverVote("v4", { height: 0, id: "a", round: 0, src: "v1", step: "Prevote" })
    })
    .then(N4F2::deliverVote("v4", { height: 0, id: "a", round: 0, src: "v2", step: "Prevote" }))
    .then(N4F2::deliverVote("v3", { height: 0, id: "b", round: 0, src: "v1", step: "Prevote" }))
    .then(N4F2::deliverVote("v3", { height: 0, id: "b", round: 0, src: "v2", step: "Prevote" }))
    .then(N4F2::deliverVote("v3", { height: 0, id: "b", round: 0, src: "v3", step: "Prevote" }))
    .then(N4F2::deliverVote("v4", { height: 0, id: "a", round: 0, src: "v4", step: "Prevote" }))
    .then(N4F2::valStep("v3"))  
    .then(N4F2::valStep("v3"))   
    .then(N4F2::valStep("v3"))
    .then(N4F2::valStep("v4"))
    .then(N4F2::valStep("v4"))
    .then(N4F2::valStep("v4"))
    .then(all{
        // they precommited diN4F2N4F2erently
        assert( N4F2::voteBuffer.get("v3").contains({ height: 0, id: "a", round: 0, src: "v4", step: "Precommit" }) and
                N4F2::voteBuffer.get("v4").contains({ height: 0, id: "b", round: 0, src: "v3", step: "Precommit" })), 
        N4F2::deliverVote("v4", { height: 0, id: "a", round: 0, src: "v1", step: "Precommit" }) })
    .then(N4F2::deliverVote("v4", { height: 0, id: "a", round: 0, src: "v2", step: "Precommit" }))    
    .then(N4F2::deliverVote("v3", { height: 0, id: "b", round: 0, src: "v1", step: "Precommit" })) 
    .then(N4F2::deliverVote("v3", { height: 0, id: "b", round: 0, src: "v2", step: "Precommit" })) 
    .then(N4F2::valStep("v3"))  
    .then(N4F2::valStep("v3"))   
    .then(N4F2::valStep("v4"))
    .then(N4F2::valStep("v4"))
    .then(all{
        assert(N4F2::AgreementInv),
        N4F2::deliverVote("v4", { height: 0, id: "a", round: 0, src: "v4", step: "Precommit" }) })
    .then(N4F2::deliverVote("v3", { height: 0, id: "b", round: 0, src: "v3", step: "Precommit" }))
    .then(N4F2::valStep("v3"))   
    .then(N4F2::valStep("v4"))
    .then(all{
        assert(not(N4F2::AgreementInv)),
        N4F2::unchangedAll})   
}



}