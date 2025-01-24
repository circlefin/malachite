module heightLogicTest {

import heightLogicrun (
    validators = Set("v1", "v2", "v3", "v4"),
    validatorSet = Map("v1" -> 1, "v2" -> 1, "v3" -> 1, "v4" -> 1),
    Faulty = Set(),
    Values = Set("a", "b", "c"),
    Rounds = Set(0, 1, 2, 3),
    Heights = Set(0, 1), 
    slow = "v4"
) as SLOW1 from "./heightLogicrun"

run fastProcessesDecidesAtHeight0SlowOneCatchesUpTest = {
    SLOW1::fastProcessesDecidesAtHeight0SlowOneCatchesUp
}

run everyoneIsFastInRound0Test = {
    SLOW1::everyoneIsFastInRound0
}

run multiHeightRunTest = {
    SLOW1::multiHeightRun
}

run slowProcessAtHeight1Test = {
    SLOW1::slowProcessAtHeight1
}

run catchUpToHigherRoundTest = {
    SLOW1::catchUpToHigherRound
}

run catchUpToHigherRoundAfterHeightChangeTest = {
    SLOW1::catchUpToHigherRoundAfterHeightChange
}

run preloadVotekeeperOnlyTest = {
    SLOW1::preloadVotekeeperOnly
}
}