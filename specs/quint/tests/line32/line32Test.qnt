// -*- mode: Bluespec; -*-

module line32Test {

import line32run(
    validators = Set("v1", "v2", "v3", "v4"),
    validatorSet = Set("v1", "v2", "v3", "v4").mapBy(x => 1),
    Faulty = Set("v1", "v2", "v3"),
    Values = Set("red", "blue", "invalid"),
    Rounds = Set(0, 1, 2, 3),
    Heights = Set(0), // , 1, 2, 3)
    testedVal = "v4",
    slowByz = "v3"
) as N4F3 from "./line32run"

run line32InvalidTest = {
    N4F3::runToLine32invalid
}

import line32run(
    validators = Set("v1", "v2", "v3", "v4"),
    validatorSet = Set("v1", "v2", "v3", "v4").mapBy(x => 1),
    Faulty = Set("v1"),
    Values = Set("red", "blue"),
    Rounds = Set(0, 1, 2, 3),
    Heights = Set(0), // , 1, 2, 3)
    testedVal = "v4",
    slowByz = "v1"
) as N4F1 from "./line32run"

run line32LockedTest = {
    N4F1::runToLine32locked
}

}