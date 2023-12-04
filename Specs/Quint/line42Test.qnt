// -*- mode: Bluespec; -*-

module line42Test {

import line42run(
    validators = Set("v1", "v2", "v3", "v4"),
    validatorSet = Set("v1", "v2", "v3", "v4").mapBy(x => 1),
    Faulty = Set(),
    Values = Set("red", "blue"),
    Rounds = Set(0, 1, 2, 3),
    Heights = Set(0), // , 1, 2, 3)
    testedVal = "v4"
) as N4F0 from "./line42run"

run line42Test = {
    N4F0::runToLine42
}

}