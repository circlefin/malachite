# Tests for consensus statemachine (and sometimes driver)

## Overview over covered consensus algorithm lines

| line | comment | (C) | test |
|  -----:| ---- | -----| -----| 
 16 |  reuse valid value | | line28Test.qnt
 18 | new value | X2 
 19 | send proposal | | (A) RoundswitchTest (^1)
 21 | start timeoutPropose | X1, X2b, X2c
 24 | prevote value | X1, X2
 26 | prevote nil (on invalid or locked) | X2c
 30 | prevote value on old prevotes | | line28Test.qnt
 32 | prevote nil on old prevotes (on invalid or locked)  |
 35 | start timeoutPrevote  | X2
 40 | precommit value  | X1
 42 without 41  | set valid without locked  | | line42Test
 45 | precommit nil | X2b
 48 | start timeoutPrecommit  | X2 , X2b
 51 | decide  | X1
 56 | skip round  | | (A) RoundswitchTest
 57 | OnTimeoutPropose  | X2b
 61 | OnTimeOutPrevote  | X2
 64 | OnTimeOutPrecommit  | X2, X2b
 
## Comments

- (C) 
    - refers to DecideNonProposerTest in consensusTest.qnt. 
    - X1 covered in height 1, X2b: covered in height 2 round 2, etc.
    - is only containing the consensus state machine. No driver
    - contains an event to go to height 1
- (A) asyncModelsTest.qnt
- ^1 ThreeDecideInRound1V4stillinZeroTest delivers proposal so it must have been sent before


## Other tests

- asyncModelsTest.qnt
    - DisagreementTest: 2/4 faulty lead to disagreement
    - three processes go to round 1 and decide, on process left behind. Test whether process decides
        - DecideForFutureRoundTest: first receives proposal then precommits
        - DecideOnProposalTest: first receives precommits then proposal