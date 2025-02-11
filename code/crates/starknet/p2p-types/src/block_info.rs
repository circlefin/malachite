use malachitebft_proto::{Error as ProtoError, Protobuf};
use malachitebft_starknet_p2p_proto::{self as p2p_proto};

use crate::proposal_commitment::L1DataAvailabilityMode;
use crate::{Address, Height};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockInfo {
    pub height: Height,
    pub builder: Address,
    pub timestamp: u64,
    pub l1_gas_price_wei: u128,
    pub l1_data_gas_price_wei: u128,
    pub l2_gas_price_fri: u128,
    pub eth_to_strk_rate: u128,
    pub l1_da_mode: L1DataAvailabilityMode,
}

impl Protobuf for BlockInfo {
    type Proto = p2p_proto::BlockInfo;

    fn from_proto(proto: Self::Proto) -> Result<Self, ProtoError> {
        Ok(Self {
            height: Height::new(proto.block_number, proto.fork_id),
            builder: Address::from_proto(
                proto
                    .builder
                    .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("builder"))?,
            )?,
            timestamp: proto.timestamp,
            l1_gas_price_wei: proto
                .l1_gas_price_wei
                .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("l1_gas_price_wei"))?
                .into(),
            l1_data_gas_price_wei: proto
                .l1_data_gas_price_wei
                .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("l1_data_gas_price_wei"))?
                .into(),
            l2_gas_price_fri: proto
                .l2_gas_price_fri
                .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("l2_gas_price_fri"))?
                .into(),
            eth_to_strk_rate: proto
                .eth_to_strk_rate
                .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("eth_to_strk_rate"))?
                .into(),
            l1_da_mode: L1DataAvailabilityMode::from_proto(proto.l1_da_mode)?,
        })
    }

    fn to_proto(&self) -> Result<Self::Proto, ProtoError> {
        Ok(Self::Proto {
            block_number: self.height.block_number,
            fork_id: self.height.fork_id,
            builder: Some(self.builder.to_proto()?),
            timestamp: self.timestamp,
            l2_gas_price_fri: Some(self.l2_gas_price_fri.into()),
            l1_gas_price_wei: Some(self.l1_gas_price_wei.into()),
            l1_data_gas_price_wei: Some(self.l1_data_gas_price_wei.into()),
            eth_to_strk_rate: Some(self.eth_to_strk_rate.into()),
            l1_da_mode: self.l1_da_mode.to_proto()?,
        })
    }
}
