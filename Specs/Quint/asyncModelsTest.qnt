// -*- mode: Bluespec; -*-

module asyncModelsTest {

import types.* from "./types"
import statemachineAsync(
    validators = Set("v1", "v2", "v3", "v4"),
    validatorSet = Set("v1", "v2", "v3", "v4").mapBy(x => 1),
    Faulty = Set(),
    Values = Set("a", "b"),
    Rounds = Set(0, 1, 2, 3),
    Heights = Set(0) // , 1, 2, 3)
) as N4F0 from "./statemachineAsync"

run ThreeDecideInRound1V4stillinZeroTest = {
    N4F0::init
    .then(N4F0::setNextValueToPropose("v2", "block"))
    .then(N4F0::valStep("v1"))
    .then(N4F0::valStep("v2"))
    .then(N4F0::valStep("v3"))
    .then(N4F0::valStep("v4"))
    .then(N4F0::valStep("v1"))
    .then(N4F0::deliverProposal("v2", mkProposal("v2", 0, 0, "block", -1)))
    .then(N4F0::valStep("v2"))
    .then(N4F0::valStep("v3")) 
    .then(N4F0::deliverVote("v1", mkVote(Prevote, "v1", 0, 0, Nil)))
    .then(N4F0::deliverVote("v1", mkVote(Prevote, "v2", 0, 0, Val("block"))))
    .then(N4F0::deliverVote("v1", mkVote(Prevote, "v3", 0, 0, Nil)))
    .then(N4F0::deliverVote("v2", mkVote(Prevote, "v1", 0, 0, Nil)))
    .then(N4F0::deliverVote("v2", mkVote(Prevote, "v2", 0, 0, Val("block"))))
    .then(N4F0::deliverVote("v2", mkVote(Prevote, "v3", 0, 0, Nil)))
    .then(N4F0::deliverVote("v3", mkVote(Prevote, "v1", 0, 0, Nil)))
    .then(N4F0::deliverVote("v3", mkVote(Prevote, "v2", 0, 0, Val("block"))))
    .then(N4F0::deliverVote("v3", mkVote(Prevote, "v3", 0, 0, Nil)))
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
    .then(N4F0::deliverVote("v1", mkVote(Precommit, "v1", 0, 0, Nil)))
    .then(N4F0::deliverVote("v1", mkVote(Precommit, "v2", 0, 0, Nil)))
    .then(N4F0::deliverVote("v1", mkVote(Precommit, "v3", 0, 0, Nil)))
    .then(N4F0::deliverVote("v2", mkVote(Precommit, "v1", 0, 0, Nil)))
    .then(N4F0::deliverVote("v2", mkVote(Precommit, "v2", 0, 0, Nil)))
    .then(N4F0::deliverVote("v2", mkVote(Precommit, "v3", 0, 0, Nil)))
    .then(N4F0::deliverVote("v3", mkVote(Precommit, "v1", 0, 0, Nil)))
    .then(N4F0::deliverVote("v3", mkVote(Precommit, "v2", 0, 0, Nil)))
    .then(N4F0::deliverVote("v3", mkVote(Precommit, "v3", 0, 0, Nil)))
    .then(N4F0::valStep("v1"))
    .then(N4F0::valStep("v2"))
    .then(N4F0::valStep("v3")) 
    .then(N4F0::valStep("v1"))
    .then(N4F0::valStep("v2"))
    .then(N4F0::valStep("v3")) 
    .then(N4F0::valStep("v1"))
    .then(N4F0::valStep("v2"))
    .then(N4F0::valStep("v3"))   
    .then(N4F0::setNextValueToPropose("v3", "another block")) 
    .then(N4F0::valStep("v1"))
    .then(N4F0::valStep("v2"))
    .then(N4F0::valStep("v3"))     
    .then(N4F0::deliverProposal("v1", mkProposal("v3", 0, 1, "another block", -1)))
    .then(N4F0::deliverProposal("v2", mkProposal("v3", 0, 1, "another block", -1)))
    .then(N4F0::deliverProposal("v3", mkProposal("v3", 0, 1, "another block", -1)))
    .then(N4F0::valStep("v1"))
    .then(N4F0::valStep("v2"))
    .then(N4F0::valStep("v3")) 
    .then(N4F0::deliverVote("v1", mkVote(Prevote, "v1", 0, 1, Val("another block"))))
    .then(N4F0::deliverVote("v1", mkVote(Prevote, "v2", 0, 1, Val("another block"))))
    .then(N4F0::deliverVote("v1", mkVote(Prevote, "v3", 0, 1, Val("another block"))))
    .then(N4F0::deliverVote("v2", mkVote(Prevote, "v1", 0, 1, Val("another block"))))
    .then(N4F0::deliverVote("v2", mkVote(Prevote, "v2", 0, 1, Val("another block"))))
    .then(N4F0::deliverVote("v2", mkVote(Prevote, "v3", 0, 1, Val("another block"))))
    .then(N4F0::deliverVote("v3", mkVote(Prevote, "v1", 0, 1, Val("another block"))))
    .then(N4F0::deliverVote("v3", mkVote(Prevote, "v2", 0, 1, Val("another block"))))
    .then(N4F0::deliverVote("v3", mkVote(Prevote, "v3", 0, 1, Val("another block"))))
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
    .then(N4F0::deliverVote("v1", mkVote(Precommit, "v1", 0, 1, Val("another block"))))
    .then(N4F0::deliverVote("v1", mkVote(Precommit, "v2", 0, 1, Val("another block"))))
    .then(N4F0::deliverVote("v1", mkVote(Precommit, "v3", 0, 1, Val("another block"))))
    .then(N4F0::deliverVote("v2", mkVote(Precommit, "v1", 0, 1, Val("another block"))))
    .then(N4F0::deliverVote("v2", mkVote(Precommit, "v2", 0, 1, Val("another block"))))
    .then(N4F0::deliverVote("v2", mkVote(Precommit, "v3", 0, 1, Val("another block"))))
    .then(N4F0::deliverVote("v3", mkVote(Precommit, "v1", 0, 1, Val("another block"))))
    .then(N4F0::deliverVote("v3", mkVote(Precommit, "v2", 0, 1, Val("another block"))))
    .then(N4F0::deliverVote("v3", mkVote(Precommit, "v3", 0, 1, Val("another block"))))
    .then(N4F0::valStep("v1"))
    .then(N4F0::valStep("v2"))
    .then(N4F0::valStep("v3")) 
    .then(N4F0::valStep("v1"))
    .then(N4F0::valStep("v2"))
    .then(N4F0::valStep("v3")) 
    .then(N4F0::valStep("v1"))
    .then(N4F0::valStep("v2"))
    .then(N4F0::valStep("v3"))
    .then(all {
        assert(N4F0::system.get("v3").es.chain.head() == Val("another block")),
        N4F0::unchangedAll
    })       
}

run DecideForFutureRoundTest = {
    ThreeDecideInRound1V4stillinZeroTest
    .then(N4F0::deliverProposal("v4", mkProposal("v3", 0, 1, "another block", -1)))
    .then(N4F0::deliverVote("v4", mkVote(Precommit, "v1", 0, 1, Val("another block"))))
    .then(N4F0::deliverVote("v4", mkVote(Precommit, "v2", 0, 1, Val("another block"))))
    .then(N4F0::deliverVote("v4", mkVote(Precommit, "v3", 0, 1, Val("another block"))))
//    .then(4.reps(i => N4F0::valStep("v4")))
    .then(N4F0::valStep("v4"))
    .then(N4F0::valStep("v4"))
    .then(N4F0::valStep("v4"))
    .then(N4F0::valStep("v4"))
    .then(all {
        assert(N4F0::system.get("v4").es.chain.head() == Val("another block")),
        N4F0::unchangedAll
    })
   // .then(N4F0::valStep("v4"))
}

run DecideOnProposalTest = {
    ThreeDecideInRound1V4stillinZeroTest
    .then(N4F0::deliverVote("v4", mkVote(Precommit, "v1", 0, 1, Val("another block"))))
    .then(N4F0::deliverVote("v4", mkVote(Precommit, "v2", 0, 1, Val("another block"))))
    .then(N4F0::deliverVote("v4", mkVote(Precommit, "v3", 0, 1, Val("another block"))))
    .then(N4F0::valStep("v4"))
    .then(N4F0::valStep("v4"))
    .then(N4F0::valStep("v4"))
    .then(N4F0::deliverProposal("v4", mkProposal("v3", 0, 1, "another block", -1)))
    .then(all {
        assert(N4F0::system.get("v4").es.chain == List()),
        N4F0::valStep("v4")
    })
    .then(all {
        assert(N4F0::system.get("v4").es.chain.head() == Val("another block")),
        N4F0::unchangedAll
    })   
}    

run RoundswitchTest = {
    ThreeDecideInRound1V4stillinZeroTest
    .then(N4F0::deliverVote("v4", mkVote(Precommit, "v1", 0, 1, Val("another block"))))
    .then(N4F0::deliverVote("v4", mkVote(Prevote, "v2", 0, 1, Val("another block"))))
    .then(N4F0::valStep("v4"))
    .then(all {
        assert(N4F0::system.get("v4").es.cs.round == 0),
        N4F0::valStep("v4")})
    .then(all {
        assert(N4F0::system.get("v4").es.cs.round == 1),
        N4F0::unchangedAll
    })   
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

import statemachineAsync(
    validators = Set("v1", "v2", "v3", "v4"),
    validatorSet = Set("v1", "v2", "v3", "v4").mapBy(x => 1),
    Faulty = Set("v1", "v2"),
    Values = Set("a", "b"),
    Rounds = Set(0),  // , 1, 2, 3)
    Heights = Set(0) // , 1, 2, 3)
) as N4F2 from "./statemachineAsync"

// v3 and v4 are correct. v2 is a faulty proposal and proposes differently to v3 and v4
// this run leads to disagreement
run DisagreementTest = {
    N4F2::init
    .then(N4F2::valStep("v3"))
    .then(N4F2::valStep("v4"))
    .then(N4F2::deliverProposal("v3", mkProposal("v2", 0, 0, "b", -1)))
    .then(N4F2::deliverProposal("v4", mkProposal("v2", 0, 0, "a", -1)))
    .then(N4F2::valStep("v3"))
    .then(N4F2::valStep("v4"))
    .then(all{
        // they voted differently
        assert(N4F2::voteBuffer == Map(
            "v3" -> Set(mkVote(Prevote, "v4", 0, 0, Val("a")), mkVote(Prevote, "v3", 0, 0, Val("b"))),
            "v4" -> Set(mkVote(Prevote, "v4", 0, 0, Val("a")), mkVote(Prevote, "v3", 0, 0, Val("b"))))),
        N4F2::deliverVote("v4", mkVote(Prevote, "v1", 0, 0, Val("a")))
    })
    .then(N4F2::deliverVote("v4", mkVote(Prevote, "v2", 0, 0, Val("a"))))
    .then(N4F2::deliverVote("v3", mkVote(Prevote, "v1", 0, 0, Val("b"))))
    .then(N4F2::deliverVote("v3", mkVote(Prevote, "v2", 0, 0, Val("b"))))
    .then(N4F2::deliverVote("v3", mkVote(Prevote, "v3", 0, 0, Val("b"))))
    .then(N4F2::deliverVote("v4", mkVote(Prevote, "v4", 0, 0, Val("a"))))
    .then(N4F2::valStep("v3"))  
    .then(N4F2::valStep("v3"))   
    .then(N4F2::valStep("v3"))
    .then(N4F2::valStep("v3"))
    .then(N4F2::valStep("v4"))
    .then(N4F2::valStep("v4"))
    .then(N4F2::valStep("v4"))
    .then(N4F2::valStep("v4"))
    .then(all{
        // they precommited diN4F2N4F2erently
        assert( N4F2::voteBuffer.get("v3").contains(mkVote(Precommit, "v4", 0, 0, Val("a"))) and
                N4F2::voteBuffer.get("v4").contains(mkVote(Precommit, "v3", 0, 0, Val("b")))), 
        N4F2::deliverVote("v4", mkVote(Precommit, "v1", 0, 0, Val("a"))) })
    .then(N4F2::deliverVote("v4", mkVote(Precommit, "v2", 0, 0, Val("a"))))
    .then(N4F2::deliverVote("v3", mkVote(Precommit, "v1", 0, 0, Val("b")))) 
    .then(N4F2::deliverVote("v3", mkVote(Precommit, "v2", 0, 0, Val("b")))) 
    .then(N4F2::valStep("v3"))  
    .then(N4F2::valStep("v3"))   
    .then(N4F2::valStep("v4"))
    .then(N4F2::valStep("v4"))
    .then(all{
        assert(N4F2::AgreementInv),
        N4F2::deliverVote("v4", mkVote(Precommit, "v4", 0, 0, Val("a"))) })
    .then(N4F2::deliverVote("v3", mkVote(Precommit, "v3", 0, 0, Val("b"))))
    .then(N4F2::valStep("v3"))   
    .then(N4F2::valStep("v4"))
    .then(all{
        assert(not(N4F2::AgreementInv)),
        N4F2::unchangedAll})   
}


}
