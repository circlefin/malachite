// -*- mode: Bluespec; -*-

module consensusTest {

    import types.* from "./types"
    import consensus.* from "./consensus"

    // *************************************************************************
    // Consensus state machine
    // *************************************************************************

    var system: Address -> ConsensusState
    var _input: (Address, ConsensusInput)
    var _output: ConsensusOutput

    action initFor(processes: Set[Address]): bool = all {
        system' = processes.mapBy(p => initConsensusState(p, 1)),
        _input' = (noAddress, NoConsensusInput),
        _output' = NoConsensusOutput,
    }

    action init = initFor(Set("Josef"))

    // For testing.
    action fireInput(proc: Address, input: ConsensusInput): bool =
        val res = consensus(system.get(proc), input)
        all {
            system' = system.put(proc, res.cs),
            _input' = (proc, input),
            _output' = res.out,
        }

    action step =
        nondet h = 1 //oneOf(1.to(4))
        nondet r = 0 //oneOf(1.to(4))
        nondet v = oneOf(Set("A", "B", "C"))
        nondet vr = oneOf(Set(-1, 1, 2, 3, 4))
        any {
            fireInput("Josef", NewHeightCInput(h)),
            fireInput("Josef", NewRoundCInput(r)),
            fireInput("Josef", NewRoundProposerCInput((r, v))),
            fireInput("Josef", ProposalCInput((r, Val(v)))),
            fireInput("Josef", ProposalAndPolkaPreviousAndValidCInput((Val(v), r))),
            fireInput("Josef", ProposalInvalidCInput),
            fireInput("Josef", PolkaNilCInput),
            fireInput("Josef", PolkaAnyCInput),
            fireInput("Josef", ProposalAndPolkaAndValidCInput(Val(v))),
            fireInput("Josef", PrecommitAnyCInput),
            fireInput("Josef", ProposalAndCommitAndValidCInput(Val(v))),
            fireInput("Josef", RoundSkipCInput(r)),
            fireInput("Josef", TimeoutProposeCInput((h, r))),
            fireInput("Josef", TimeoutPrevoteCInput((h, r))),
            fireInput("Josef", TimeoutPrecommitCInput((h, r))),
            fireInput("Josef", ProposalAndPolkaAndInvalidCInput((h, r, Val(v)))),
        }

    action unchangedAll = all {
        system' = system,
        _input' = _input,
        _output' = _output,
    }

    // *************************************************************************
    // Tests
    // *************************************************************************

    // For testing.
    action _assert(predicate: bool): bool =
        all { assert(predicate), unchangedAll }

    // This test should call each input type at least once
    run DecideNonProposerTest =
        initFor(Set("Josef"))
        .then(fireInput("Josef", NewRoundCInput(0)))
        .then(_assert(_output == TimeoutOutput(ProposeTimeout)))
        
        .then(fireInput("Josef", ProposalCInput((0, Val("block")))))
        .then(_assert(_output.isVoteMsgWith(Prevote, Val("block"))))
        
        .then(fireInput("Josef", ProposalAndPolkaAndValidCInput(Val("block"))))
        .then(_assert(_output.isVoteMsgWith(Precommit, Val("block"))))
        
        .then(fireInput("Josef", ProposalAndCommitAndValidCInput(Val("block"))))
        .then(_assert(_output == DecidedOutput(Val("block"))))
        
        .then(fireInput("Josef", NewHeightCInput(system.get("Josef").height + 1)))
        .then(_assert(system.get("Josef").height == 2))
        
        .then(fireInput("Josef", NewRoundProposerCInput((0, "nextBlock"))))
        .then(_assert(_output == ProposalOutput(mkProposal("Josef", 2, 0, "nextBlock", -1))))
        
        .then(fireInput("Josef", ProposalCInput((0, Val("nextBlock"))))) // it is assumed that the proposer receives its own message
        .then(_assert(_output.isVoteMsgWith(Prevote, Val("nextBlock")) and system.get("Josef").step == PrevoteStep))
        
        .then(fireInput("Josef", PolkaAnyCInput))
        .then(_assert(_output == TimeoutOutput(PrevoteTimeout)))
        
        .then(fireInput("Josef", TimeoutPrevoteCInput((2, 0))))
        .then(_assert(_output.isVoteMsgWith(Precommit, Nil) and system.get("Josef").step == PrecommitStep))
        
        .then(fireInput("Josef", PrecommitAnyCInput))
        .then(_assert(_output == TimeoutOutput(PrecommitTimeout)))

        .then(fireInput("Josef", TimeoutPrecommitCInput((2, 0))))
        .then(_assert(_output == SkipRoundOutput(1)))
        
        .then(fireInput("Josef", NewRoundCInput(1)))
        .then(_assert(_output == TimeoutOutput(ProposeTimeout)))

        .then(fireInput("Josef", TimeoutProposeCInput((2, 1))))
        .then(_assert(_output.isVoteMsgWith(Prevote, Nil) and system.get("Josef").step == PrevoteStep))
        
        .then(fireInput("Josef", PolkaNilCInput))
        .then(_assert(_output.isVoteMsgWith(Precommit, Nil) and system.get("Josef").step == PrecommitStep))
        
        .then(fireInput("Josef", PrecommitAnyCInput))
        .then(_assert(_output == TimeoutOutput(PrecommitTimeout)))
        
        .then(fireInput("Josef", TimeoutPrecommitCInput((2, 1))))
        .then(_assert(_output == SkipRoundOutput(2)))
        
        .then(fireInput("Josef", NewRoundCInput(2)))
        .then(_assert(_output == TimeoutOutput(ProposeTimeout)))
        
        .then(fireInput("Josef", ProposalInvalidCInput))
        .then(_assert(_output.isVoteMsgWith(Prevote, Nil) and system.get("Josef").step == PrevoteStep))

}
