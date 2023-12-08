// -*- mode: Bluespec; -*-

module consensusTest {

    import types.* from "./types"
    import consensus.* from "./consensus"

    // *************************************************************************
    // Consensus state
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
            fireInput("Josef", NewHeightConInput(h)),
            fireInput("Josef", NewRoundConInput(r)),
            fireInput("Josef", NewRoundProposerConInput((r, v))),
            fireInput("Josef", ProposalConInput((r, Val(v)))),
            fireInput("Josef", ProposalAndPolkaPreviousAndValidConInput((Val(v), r))),
            fireInput("Josef", ProposalInvalidConInput),
            fireInput("Josef", PolkaNilConInput),
            fireInput("Josef", PolkaAnyConInput),
            fireInput("Josef", ProposalAndPolkaAndValidConInput(Val(v))),
            fireInput("Josef", PrecommitAnyConInput),
            fireInput("Josef", ProposalAndCommitAndValidConInput(Val(v))),
            fireInput("Josef", RoundSkipConInput(r)),
            fireInput("Josef", TimeoutProposeConInput((h, r))),
            fireInput("Josef", TimeoutPrevoteConInput((h, r))),
            fireInput("Josef", TimeoutPrecommitConInput((h, r))),
            fireInput("Josef", ProposalAndPolkaAndInvalidConInput((h, r, Val(v)))),
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
        .then(fireInput("Josef", NewRoundConInput(0)))
        .then(_assert(_output == TimeoutOutput(ProposeTimeout)))
        
        .then(fireInput("Josef", ProposalConInput((0, Val("block")))))
        .then(_assert(_output.isVoteMsgWith(Prevote, Val("block"))))
        
        .then(fireInput("Josef", ProposalAndPolkaAndValidConInput(Val("block"))))
        .then(_assert(_output.isVoteMsgWith(Precommit, Val("block"))))
        
        .then(fireInput("Josef", ProposalAndCommitAndValidConInput(Val("block"))))
        .then(_assert(_output == DecidedOutput(Val("block"))))
        
        .then(fireInput("Josef", NewHeightConInput(system.get("Josef").height + 1)))
        .then(_assert(system.get("Josef").height == 2))
        
        .then(fireInput("Josef", NewRoundProposerConInput((0, "nextBlock"))))
        .then(_assert(_output == ProposalOutput(mkProposal("Josef", 2, 0, "nextBlock", -1))))
        
        .then(fireInput("Josef", ProposalConInput((0, Val("nextBlock"))))) // it is assumed that the proposer receives its own message
        .then(_assert(_output.isVoteMsgWith(Prevote, Val("nextBlock")) and system.get("Josef").step == PrevoteStep))
        
        .then(fireInput("Josef", PolkaAnyConInput))
        .then(_assert(_output == TimeoutOutput(PrevoteTimeout)))
        
        .then(fireInput("Josef", TimeoutPrevoteConInput((2, 0))))
        .then(_assert(_output.isVoteMsgWith(Precommit, Nil) and system.get("Josef").step == PrecommitStep))
        
        .then(fireInput("Josef", PrecommitAnyConInput))
        .then(_assert(_output == TimeoutOutput(PrecommitTimeout)))

        .then(fireInput("Josef", TimeoutPrecommitConInput((2, 0))))
        .then(_assert(_output == SkipRoundOutput(1)))
        
        .then(fireInput("Josef", NewRoundConInput(1)))
        .then(_assert(_output == TimeoutOutput(ProposeTimeout)))

        .then(fireInput("Josef", TimeoutProposeConInput((2, 1))))
        .then(_assert(_output.isVoteMsgWith(Prevote, Nil) and system.get("Josef").step == PrevoteStep))
        
        .then(fireInput("Josef", PolkaNilConInput))
        .then(_assert(_output.isVoteMsgWith(Precommit, Nil) and system.get("Josef").step == PrecommitStep))
        
        .then(fireInput("Josef", PrecommitAnyConInput))
        .then(_assert(_output == TimeoutOutput(PrecommitTimeout)))
        
        .then(fireInput("Josef", TimeoutPrecommitConInput((2, 1))))
        .then(_assert(_output == SkipRoundOutput(2)))
        
        .then(fireInput("Josef", NewRoundConInput(2)))
        .then(_assert(_output == TimeoutOutput(ProposeTimeout)))
        
        .then(fireInput("Josef", ProposalInvalidConInput))
        .then(_assert(_output.isVoteMsgWith(Prevote, Nil) and system.get("Josef").step == PrevoteStep))

}
