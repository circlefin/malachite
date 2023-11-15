use itf::ItfBigInt;
use serde::de::IntoDeserializer;
use serde::Deserialize;

use crate::consensus::{Proposal, VoteMessage};

pub(crate) fn empty_string_as_none<'de, D, T>(de: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    let opt = Option::<String>::deserialize(de)?;
    match opt.as_deref() {
        None | Some("") => Ok(None),
        Some(s) => T::deserialize(s.into_deserializer()).map(Some),
    }
}

pub(crate) fn minus_one_as_none<'de, D>(de: D) -> Result<Option<ItfBigInt>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<ItfBigInt>::deserialize(de)?;
    match opt {
        None => Ok(None),
        Some(i) if i == ItfBigInt::from(-1) => Ok(None),
        Some(i) => Ok(Some(i)),
    }
}

pub(crate) fn proposal_or_none<'de, D>(de: D) -> Result<Option<Proposal>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let proposal = Proposal::deserialize(de)?;
    if proposal.is_empty() {
        Ok(None)
    } else {
        Ok(Some(proposal))
    }
}

pub(crate) fn vote_message_or_none<'de, D>(de: D) -> Result<Option<VoteMessage>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let vote_message = VoteMessage::deserialize(de)?;
    if vote_message.is_empty() {
        Ok(None)
    } else {
        Ok(Some(vote_message))
    }
}
