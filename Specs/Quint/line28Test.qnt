// -*- mode: Bluespec; -*-

module line28Test {

import line28run(
    validators = Set("v1", "v2", "v3", "v4"),
    validatorSet = Set("v1", "v2", "v3", "v4").mapBy(x => 1),
    Faulty = Set("v1"),
    Values = Set("red", "blue"),
    Rounds = Set(0, 1, 2, 3),
    Heights = Set(0), // , 1, 2, 3)
    otherSet = Set("v2", "v4")
) as N4F1 from "./line28run"

run line28Test = {
    N4F1::runToLine28
}

import line28run(
    validators = Set("v1", "v2", "v3", "v4", "v5", "v6", "v7"),
    validatorSet = Set("v1", "v2", "v3", "v4", "v5", "v6", "v7").mapBy(x => 1),
    Faulty = Set("v1"),
    Values = Set("red", "blue"),
    Rounds = Set(0, 1, 2, 3),
    Heights = Set(0), // , 1, 2, 3)
    otherSet = Set("v2", "v4", "v6", "v7")
) as N7F1 from "./line28run"

run Bigline28Test = {
    N7F1::runToLine28
}


}