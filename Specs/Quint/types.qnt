// -*- mode: Bluespec; -*-

module types {

    // A node address
    type Address = str
    val noAddress: Address = ""

    // Height in the Tendermint algorithm
    type Height = int

    // Round in the Tendermint algorithm
    type Round = int

    // Value proposed and voted to be included in the chain
    // A value is either nil or a string
    type NonNilValue = str
    type Value = Nil | Val(NonNilValue)
    pure def getVal(value: Value): NonNilValue = 
        match value {
            | Val(v) => v
            | Nil => "getVal error" // this should not happen
        }
    pure def isValid(value: NonNilValue): bool =
        // for simulation, if the value is "invalid", so is the proposal
        // if this is to become "non-deterministic", we must push it
        // and its use into the state machine
        value != "invalid"

    // The stake of an address
    type Weight = int

    // Names of round steps
    type Step =
        | NewRoundStep
        | ProposeStep
        | PrevoteStep
        | PrecommitStep
        | DecidedStep

    type Timeout =
        | ProposeTimeout // Timeout waiting for proposal.
        | PrevoteTimeout // Timeout waiting for prevotes for a value.
        | PrecommitTimeout // Timeout waiting for precommits for a value.

    type ValueId = Value
    
    // efficient hashing function
    // pure def id(value: Value): ValueId = value
    pure def id(value) = value

    // *************************************************************************
    // Messages
    // *************************************************************************

    type Proposal = {
        srcAddress: Address,
        height: Height,
        round: Round,
        proposal: NonNilValue, // an actual value. All other values are id(proposal)
        validRound: Round,
    }
    
    def mkProposal(src, height, round, value, validRound) = 
        { srcAddress: src, height: height, round: round, proposal: value, validRound: validRound }

    type VoteType = Prevote | Precommit
    type Vote = {
        voteType: VoteType,
        srcAddress: Address,
        height: Height,
        round: Round,
        valueId: ValueId,
    }
    pure def mkVote(voteType, srcAddress, height, round, valueId) = 
        { voteType: voteType, srcAddress: srcAddress, height: height, round: round, valueId: valueId }

}
