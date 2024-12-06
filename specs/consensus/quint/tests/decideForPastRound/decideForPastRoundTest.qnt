// -*- mode: Bluespec; -*-

module decideForPastRoundTest {

import decideForPastRound(
    validators = Set("v1", "v2", "v3", "v4"),
    validatorSet = Set("v1", "v2", "v3", "v4").mapBy(x => 1),
    Faulty = Set("v1"),
    Values = Set("red", "blue"),
    Rounds = Set(0, 1, 2, 3),
    Heights = Set(0), // , 1, 2, 3)
    otherSet = Set("v2", "v3"),
    testedVal = ("v4")
) as N4F1 from "./decideForPastRoundrun"

run decidePastTest = {
    N4F1::runToDecision
}

}
