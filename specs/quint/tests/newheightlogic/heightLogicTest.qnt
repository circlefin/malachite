module heightLogicTest {

import heightLogicrun (
    validators = Set("v1", "v2", "v3", "v4"),
    validatorSet = Map("v1" -> 1, "v2" -> 1, "v3" -> 1, "v4 " -> 1),
    Faulty = Set(),
    Values = Set("a", "b"),
    Rounds = Set(0, 1, 2, 3),
    Heights = Set(0,1), 
    slow = "v4"
) as SLOW1 from "./heightLogicrun"

run fastProcessesDecidesAtHeight0 = {
    SLOW1::fastProcessesDecidesAtHeight0
}

run deliverProposalForFast = {
    SLOW1::deliverProposalForFast
}
}