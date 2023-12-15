// -*- mode: Bluespec; -*-

module someMultiRoundTest {

import someMultiRoundRuns(
    validators = Set("v1", "v2", "v3", "v4"),
    validatorSet = Set("v1", "v2", "v3", "v4").mapBy(x => 1),
    Faulty = Set(),
    Values = Set("red", "blue"),
    Rounds = Set(0, 1, 2, 3),
    Heights = Set(0), // , 1, 2, 3)
    slow = "v4"
) as N4F0 from "./someMultiRoundRuns"

run AllButOneTest = 
    N4F0::AllButOneDecideInR1WhileSlowInR0

run DecideFutureRoundTest = 
    N4F0::DecideForFutureRoundRun

run DecideForFutureRoundPrecommitFirstTest = 
    N4F0::DecideForFutureRoundPrecommitFirstRun

run RoundswitchTest =
    N4F0::RoundswitchRun

}
