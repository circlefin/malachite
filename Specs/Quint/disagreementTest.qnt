// -*- mode: Bluespec; -*-

module disagreementTest {

import disagreementRun(
    validators = Set("v1", "v2", "v3", "v4"),
    validatorSet = Set("v1", "v2", "v3", "v4").mapBy(x => 1),
    Faulty = Set("v1", "v2"),
    Values = Set("a", "b"),
    Rounds = Set(0),  // , 1, 2, 3)
    Heights = Set(0), // , 1, 2, 3)
    groupA = Set("v3")
) as N4F2 from "./disagreementRun"

run disagreementTest = {
    N4F2::DisagreementRun    
}


}