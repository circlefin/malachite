// -*- mode: Bluespec; -*-

module line26Test {

import line26run(
    validators = Set("v1", "v2", "v3", "v4"),
    validatorSet = Set("v1", "v2", "v3", "v4").mapBy(x => 1),
    Faulty = Set("v3"), // validator of second round
    Values = Set("red", "blue"),
    Rounds = Set(0, 1, 2, 3),
    Heights = Set(0), // , 1, 2, 3)
    otherSet = Set("v2", "v4")
).* from "./line26run"

run line26Test = {
    runToLine26
}



}
