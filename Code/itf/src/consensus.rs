use itf::de::{As, Integer, Same};
use serde::Deserialize;
use std::collections::HashMap;

use crate::deserializers as de;
use crate::types::{Address, Height, NonNilValue, Proposal, Round, Step, Timeout, Value, Vote};

#[derive(Clone, Debug, Deserialize)]
pub struct State {
    pub system: System,

    #[serde(rename = "_input")]
    pub input: (Address, Input),

    #[serde(rename = "_output")]
    pub output: Output,
}

#[derive(Clone, Debug, Deserialize)]
pub struct System(pub HashMap<Address, ConsensusState>);

#[derive(Clone, Debug, Deserialize)]
#[serde(rename = "ConsensusInput")]
#[serde(tag = "tag", content = "value")]
pub enum Input {
    #[serde(rename = "NoConsensusInput")]
    NoInput,
    #[serde(rename = "NewHeightCInput")]
    #[serde(with = "As::<Integer>")]
    NewHeight(Height),
    #[serde(rename = "NewRoundCInput")]
    #[serde(with = "As::<Integer>")]
    NewRound(Round),
    #[serde(rename = "NewRoundProposerCInput")]
    #[serde(with = "As::<(Integer, Same)>")]
    NewRoundProposer(Round, NonNilValue),
    #[serde(rename = "ProposalCInput")]
    #[serde(with = "As::<(Integer, Same)>")]
    Proposal(Round, Value),
    #[serde(rename = "ProposalAndPolkaPreviousAndValidCInput")]
    #[serde(with = "As::<(Integer, Same)>")]
    ProposalAndPolkaPreviousAndValid(Round, Value),
    #[serde(rename = "ProposalInvalidCInput")]
    ProposalInvalid,
    #[serde(rename = "PolkaNilCInput")]
    PolkaNil,
    #[serde(rename = "PolkaAnyCInput")]
    PolkaAny,
    #[serde(rename = "ProposalAndPolkaAndValidCInput")]
    ProposalAndPolkaAndValid(Value),
    #[serde(rename = "PrecommitAnyCInput")]
    PrecommitAny,
    #[serde(rename = "ProposalAndCommitAndValidCInput")]
    ProposalAndCommitAndValid(Value),
    #[serde(rename = "RoundSkipCInput")]
    #[serde(with = "As::<Integer>")]
    RoundSkip(Round),
    #[serde(rename = "TimeoutProposeCInput")]
    #[serde(with = "As::<(Integer, Integer)>")]
    TimeoutPropose(Height, Round),
    #[serde(rename = "TimeoutPrevoteCInput")]
    #[serde(with = "As::<(Integer, Integer)>")]
    TimeoutPrevote(Height, Round),
    #[serde(rename = "TimeoutPrecommitCInput")]
    #[serde(with = "As::<(Integer, Integer)>")]
    TimeoutPrecommit(Height, Round),
    #[serde(rename = "ProposalAndPolkaAndInvalidCInputCInput")]
    #[serde(with = "As::<(Integer, Integer, Same)>")]
    ProposalAndPolkaAndInvalidCInput(Height, Round, Value),
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename = "ConsensusOutput")]
#[serde(tag = "tag", content = "value")]
pub enum Output {
    #[serde(rename = "NoConsensusOutput")]
    NoOutput,
    #[serde(rename = "ProposalOutput")]
    Proposal(Proposal),
    #[serde(rename = "VoteOutput")]
    Vote(Vote),
    #[serde(rename = "TimeoutOutput")]
    Timeout(Timeout),
    #[serde(rename = "DecidedOutput")]
    Decided(Value),
    #[serde(rename = "SkipRoundOutput")]
    #[serde(with = "As::<Integer>")]
    SkipRound(Round),
    #[serde(rename = "ErrorOutput")]
    Error(String),
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsensusState {
    pub p: Address,
    #[serde(with = "As::<Integer>")]
    pub height: Height,
    #[serde(with = "As::<Integer>")]
    pub round: Round,
    pub step: Step,
    #[serde(deserialize_with = "de::minus_one_as_none")]
    pub locked_round: Option<Round>,
    pub locked_value: Value,
    #[serde(deserialize_with = "de::minus_one_as_none")]
    pub valid_round: Option<Round>,
    pub valid_value: Value,
}
