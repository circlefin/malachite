// -*- mode: Bluespec; -*-

module hiddenLockTest {

import hiddenLockrun(
    validators = Set("v1", "v2", "v3", "v4"),
    validatorSet = Set("v1", "v2", "v3", "v4").mapBy(x => 1),
    Faulty = Set("v2"),
    testedval = "v1", 
    Values = Set("red", "blue"),
    Rounds = Set(0, 1, 2, 3),
    Heights = Set(0), // , 1, 2, 3)
    otherSet = Set("v4", "v3")
) as N4F1 from "./hiddenLockrun"



run hiddenTest = {
    N4F1::hiddenlockrun
}




}