// -*- mode: Bluespec; -*-

module consensusTest {

    import types.* from "../../specs/types"
    import consensus.* from "../../specs/consensus"

    // *************************************************************************
    // Consensus state machine
    // *************************************************************************

    var state: ConsensusState
    var input: ConsensusInput
    var output: ConsensusOutput

    action initFor(p: Address, height: Height): bool = all {
        state' = initConsensusState(p, height),
        input' = NoConsensusInput,
        output' = NoConsensusOutput,
    }

    action init = initFor("v1", 1)

    // For testing.
    action fireInput(__input: ConsensusInput): bool =
        val res = consensus(state, __input)
        all {
            state' = res.cs,
            input' = __input,
            output' = res.out,
        }

    action step =
        nondet h = 1 //oneOf(1.to(4))
        nondet r = 0 //oneOf(1.to(4))
        nondet v = oneOf(Set("A", "B", "C"))
        nondet vr = oneOf(Set(-1, 1, 2, 3, 4))
        any {
            fireInput(NewRoundCInput(r)),
            fireInput(NewRoundProposerCInput((r, v))),
            fireInput(ProposalCInput((r, Val(v)))),
            fireInput(ProposalAndPolkaPreviousAndValidCInput((Val(v), r))),
            fireInput(ProposalInvalidCInput),
            fireInput(PolkaNilCInput),
            fireInput(PolkaAnyCInput),
            fireInput(ProposalAndPolkaAndValidCInput(Val(v))),
            fireInput(PrecommitAnyCInput),
            fireInput(ProposalAndCommitAndValidCInput(Val(v))),
            fireInput(RoundSkipCInput(r)),
            fireInput(TimeoutProposeCInput((h, r))),
            fireInput(TimeoutPrevoteCInput((h, r))),
            fireInput(TimeoutPrecommitCInput((h, r))),
            fireInput(ProposalAndPolkaAndInvalidCInput(Val(v))),
        }

    action unchangedAll = all {
        state' = state,
        input' = input,
        output' = output,
    }

    // *************************************************************************
    // Tests
    // *************************************************************************

    // For testing.
    action _assert(predicate: bool): bool =
        all { assert(predicate), unchangedAll }

    // This test should call each input type at least once
    run DecideNonProposerTest =
        initFor("v1", 1)
        .then(fireInput(NewRoundCInput(0)))
        .expect(output == TimeoutOutput((0, ProposeTimeout)))
        
        .then(fireInput(ProposalCInput((0, Val("block")))))
        .expect(output.isVoteMsgWith(Prevote, Val("block")))
        
        .then(fireInput(ProposalAndPolkaAndValidCInput(Val("block"))))
        .expect(output.isVoteMsgWith(Precommit, Val("block")))
        
        .then(fireInput(ProposalAndCommitAndValidCInput(Val("block"))))
        .expect(output == DecidedOutput(Val("block")))

    // TODO: rename test?
    run DecideNonProposerAtHeight2Test =
        initFor("v1", 2)     
        .then(fireInput(NewRoundProposerCInput((0, "nextBlock"))))
        .expect(output == ProposalOutput(mkProposal("v1", 2, 0, "nextBlock", -1)))
        
        .then(fireInput(ProposalCInput((0, Val("nextBlock"))))) // it is assumed that the proposer receives its own message
        .expect(output.isVoteMsgWith(Prevote, Val("nextBlock")) and state.step == PrevoteStep)
        
        .then(fireInput(PolkaAnyCInput))
        .expect(output == TimeoutOutput((0, PrevoteTimeout)))
        
        .then(fireInput(TimeoutPrevoteCInput((2, 0))))
        .expect(output.isVoteMsgWith(Precommit, Nil) and state.step == PrecommitStep)
        
        .then(fireInput(PrecommitAnyCInput))
        .expect(output == TimeoutOutput((0, PrecommitTimeout)))

        .then(fireInput(TimeoutPrecommitCInput((2, 0))))
        .expect(output == SkipRoundOutput(1) and state.step == UnstartedStep)
        
        .then(fireInput(NewRoundCInput(1)))
        .expect(output == TimeoutOutput((1, ProposeTimeout)))

        .then(fireInput(TimeoutProposeCInput((2, 1))))
        .expect(output.isVoteMsgWith(Prevote, Nil) and state.step == PrevoteStep)
        
        .then(fireInput(PolkaNilCInput))
        .expect(output.isVoteMsgWith(Precommit, Nil) and state.step == PrecommitStep)
        
        .then(fireInput(PrecommitAnyCInput))
        .expect(output == TimeoutOutput((1, PrecommitTimeout)))
        
        .then(fireInput(TimeoutPrecommitCInput((2, 1))))
        .expect(output == SkipRoundOutput(2))
        
        .then(fireInput(NewRoundCInput(2)))
        .expect(output == TimeoutOutput((2, ProposeTimeout)))
        
        .then(fireInput(ProposalInvalidCInput))
        .expect(output.isVoteMsgWith(Prevote, Nil) and state.step == PrevoteStep)

}
