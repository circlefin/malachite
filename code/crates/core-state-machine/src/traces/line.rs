use displaydoc::Display;

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq, Display)]
pub enum Line {
    /// L11 - proposer
    L11Proposer,

    /// L11 - non-proposer: schedule proposeTimeout
    L11NonProposer,

    /// L14 - check if proposer
    L14,

    /// L16 - validValue
    L16,

    /// L18 - getValue()
    L18,

    /// L19 - proposal
    L19,

    /// L21 - proposeTimeout scheduled
    L21ProposeTimeoutScheduled,

    /// L22 - proposal in propose step: prevote
    L22,

    /// L24 - prevote v: valid(v) and lockedValue = v
    L24ValidAndLockedValue,

    /// L24 - prevote v: valid(v) and lockedRound == -1
    L24ValidNoLockedRound,

    /// L26 - prevote nil: valid(v) and lockedValue != v
    L26ValidAndLockedValue,

    /// L28 - valid proposal
    L28ValidProposal,

    /// L28 - invalid proposal
    L28InvalidProposal,

    /// L30 - prevote v: valid(v) and 0 <= lockedRound <= vr
    L30ValidLockedRound,

    /// L30 - prevote v: valid(v) and lockedValue = v
    L30ValidLockedValue,

    /// L30 - prevote v: valid(v) and lockedRound == -1
    L30ValidNoLockedRound,

    /// L32 - prevote nil: valid(v) and lockedRound > vr and lockedValue != v
    L32ValidLockedRound,

    /// L32 - invalid value
    L32InvalidValue,

    /// L34 - polka any: schedule prevoteTimeout
    L34,

    /// L35 - prevoteTimeout scheduled
    L35,

    /// L36 - valid v and step == prevote: set locked, valid
    L36ValidProposal,

    /// L45 - polka nil: precommit nil
    L45,

    /// L48 - precommit any: schedule precommitTimeout
    L48,

    /// L49 - valid v and precommit quorum: commit
    L49,

    /// L55 - f+1 for higher round: move to that round
    L55,

    /// L59 - proposer, proposeTimeout expired: prevote nil
    L59Proposer,

    /// L59 - non proposer, proposeTimeout expired: prevote nil
    L59NonProposer,

    /// L61 - prevoteTimeout expired: precommit nil
    L61,

    /// L67 - precommitTimeout expired: move to next round
    L67,
}
