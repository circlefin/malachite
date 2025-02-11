use malachitebft_proto::{Error as ProtoError, Protobuf};
use malachitebft_starknet_p2p_proto::{self as p2p_proto};

use crate::felt::FeltExt;
use crate::{Address, Felt, Hash, Height};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposalCommitment {
    pub height: Height,
    pub parent_commitment: Hash,
    pub builder: Address,
    pub timestamp: u64,
    pub protocol_version: String,
    pub old_state_root: Hash,
    pub state_diff_commitment: Hash,
    pub transaction_commitment: Hash,
    pub event_commitment: Hash,
    pub receipt_commitment: Hash,
    pub concatenated_counts: Felt,
    pub l1_gas_price_fri: u128,
    pub l1_data_gas_price_fri: u128,
    pub l2_gas_price_fri: u128,
    pub l2_gas_used: u128,
    pub l1_da_mode: L1DataAvailabilityMode,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum L1DataAvailabilityMode {
    Calldata = 0,
    Blob = 1,
}

impl Protobuf for L1DataAvailabilityMode {
    type Proto = i32;

    fn from_proto(proto: Self::Proto) -> Result<Self, ProtoError> {
        let proto = p2p_proto::L1DataAvailabilityMode::try_from(proto).map_err(|_| {
            ProtoError::invalid_data::<Self::Proto>("invalid value for L1DataAvailabilityMode")
        })?;

        match proto {
            p2p_proto::L1DataAvailabilityMode::Calldata => Ok(Self::Calldata),
            p2p_proto::L1DataAvailabilityMode::Blob => Ok(Self::Blob),
        }
    }

    fn to_proto(&self) -> Result<Self::Proto, ProtoError> {
        match self {
            Self::Calldata => Ok(p2p_proto::L1DataAvailabilityMode::Calldata as i32),
            Self::Blob => Ok(p2p_proto::L1DataAvailabilityMode::Blob as i32),
        }
    }
}

impl Protobuf for ProposalCommitment {
    type Proto = p2p_proto::ProposalCommitment;

    fn from_proto(proto: Self::Proto) -> Result<Self, ProtoError> {
        Ok(Self {
            height: Height::new(proto.block_number, proto.fork_id),
            parent_commitment: Hash::from_proto(
                proto
                    .parent_commitment
                    .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("parent_commitment"))?,
            )?,
            builder: Address::from_proto(
                proto
                    .builder
                    .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("builder"))?,
            )?,
            timestamp: proto.timestamp,
            protocol_version: proto.protocol_version,
            old_state_root: Hash::from_proto(
                proto
                    .old_state_root
                    .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("old_state_root"))?,
            )?,
            state_diff_commitment: Hash::from_proto(proto.state_diff_commitment.ok_or_else(
                || ProtoError::missing_field::<Self::Proto>("state_diff_commitment"),
            )?)?,
            transaction_commitment: Hash::from_proto(proto.transaction_commitment.ok_or_else(
                || ProtoError::missing_field::<Self::Proto>("transaction_commitment"),
            )?)?,
            event_commitment: Hash::from_proto(
                proto
                    .event_commitment
                    .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("event_commitment"))?,
            )?,
            receipt_commitment: Hash::from_proto(
                proto.receipt_commitment.ok_or_else(|| {
                    ProtoError::missing_field::<Self::Proto>("receipt_commitment")
                })?,
            )?,
            concatenated_counts: Felt::from_proto(
                proto.concatenated_counts.ok_or_else(|| {
                    ProtoError::missing_field::<Self::Proto>("concatenated_counts")
                })?,
            )?,
            l1_gas_price_fri: proto
                .l1_gas_price_fri
                .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("l1_gas_price_fri"))?
                .into(),
            l1_data_gas_price_fri: proto
                .l1_data_gas_price_fri
                .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("l1_data_gas_price_fri"))?
                .into(),
            l2_gas_price_fri: proto
                .l2_gas_price_fri
                .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("l2_gas_price_fri"))?
                .into(),
            l2_gas_used: proto
                .l2_gas_used
                .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("l2_gas_used"))?
                .into(),
            l1_da_mode: L1DataAvailabilityMode::from_proto(proto.l1_da_mode)?,
        })
    }

    fn to_proto(&self) -> Result<Self::Proto, ProtoError> {
        Ok(Self::Proto {
            block_number: self.height.block_number,
            fork_id: self.height.fork_id,
            parent_commitment: Some(self.parent_commitment.to_proto()?),
            builder: Some(self.builder.to_proto()?),
            timestamp: self.timestamp,
            protocol_version: self.protocol_version.clone(),
            old_state_root: Some(self.old_state_root.to_proto()?),
            state_diff_commitment: Some(self.state_diff_commitment.to_proto()?),
            transaction_commitment: Some(self.transaction_commitment.to_proto()?),
            event_commitment: Some(self.event_commitment.to_proto()?),
            receipt_commitment: Some(self.receipt_commitment.to_proto()?),
            concatenated_counts: Some(self.concatenated_counts.to_proto()?),
            l1_gas_price_fri: Some(self.l1_gas_price_fri.into()),
            l1_data_gas_price_fri: Some(self.l1_data_gas_price_fri.into()),
            l2_gas_price_fri: Some(self.l2_gas_price_fri.into()),
            l2_gas_used: Some(self.l2_gas_used.into()),
            l1_da_mode: self.l1_da_mode.to_proto()?,
        })
    }
}
